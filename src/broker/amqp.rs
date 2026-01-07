use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::BasicProperties;
use lapin::{options::*, types::FieldTable, Connection, ConnectionProperties, Consumer};
use pyo3::exceptions::PyRuntimeError;
use pyo3::PyResult;
use tokio::sync::{watch, Mutex};
use tokio::time::sleep;

use super::config::AmqpConfig;
use super::BrokerMessage;
use crate::broker::Broker;
use crate::channel_store::ChannelStore;

pub(crate) struct Amqp {
    config: AmqpConfig,
    channel: Mutex<Option<lapin::Channel>>,
}

impl Amqp {
    pub(crate) fn new(config: &AmqpConfig) -> Self {
        Self {
            config: config.clone(),
            channel: Mutex::new(None),
        }
    }

    async fn setup(&self) -> Result<Consumer, lapin::Error> {
        let conn = Connection::connect(&self.config.url, ConnectionProperties::default()).await?;
        let channel = conn.create_channel().await?;

        channel
            .exchange_declare(
                &self.config.exchange,
                lapin::ExchangeKind::Fanout,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        let queue_name = self
            .config
            .queue
            .clone()
            .unwrap_or_else(|| format!("ws_server_{}", uuid::Uuid::new_v4()));

        let queue = channel
            .queue_declare(
                &queue_name,
                QueueDeclareOptions {
                    exclusive: true,
                    auto_delete: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        channel
            .queue_bind(
                queue.name().as_str(),
                &self.config.exchange,
                &self.config.routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let consumer = channel
            .basic_consume(
                queue.name().as_str(),
                "ws_consumer",
                BasicConsumeOptions {
                    no_ack: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        Ok(consumer)
    }
}

#[async_trait]
impl Broker for Amqp {
    async fn listen(&self, channels: Arc<ChannelStore>, mut shutdown: watch::Receiver<bool>) {
        shutdown.mark_unchanged();
        let mut backoff = Duration::from_millis(100);
        const MAX_BACKOFF: Duration = Duration::from_secs(30);

        loop {
            let mut consumer = match self.setup().await {
                Ok(c) => {
                    log::info!(
                        "Connected to AMQP, consuming from exchange '{}'",
                        self.config.exchange
                    );
                    backoff = Duration::from_millis(100);
                    c
                }
                Err(e) => {
                    log::warn!("AMQP connection failed: {}, retrying in {:?}", e, backoff);
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, MAX_BACKOFF);
                    continue;
                }
            };

            loop {
                tokio::select! {
                    _ = shutdown.changed() => {
                        log::info!("AMQP listener shutting down");
                        return;
                    }
                    delivery = consumer.next() => {
                        match delivery {
                            Some(Ok(delivery)) => {
                                let payload = match std::str::from_utf8(&delivery.data) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        log::error!("AMQP payload not UTF-8: {}", e);
                                        continue;
                                    }
                                };

                                match serde_json::from_str::<BrokerMessage>(payload) {
                                    Ok(broker_msg) => {
                                        broker_msg.dispatch(&channels);
                                    }
                                    Err(e) => {
                                        log::error!("Invalid broker message format: {} - payload: {}", e, payload);
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                log::warn!("AMQP delivery error: {}, reconnecting...", e);
                                break;
                            }
                            None => {
                                log::warn!("AMQP consumer ended, reconnecting...");
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn send(&self, payload: String) -> PyResult<()> {
        let mut guard = self.channel.lock().await;

        let needs_new = match guard.as_ref() {
            Some(ch) => !ch.status().connected(),
            None => true,
        };

        if needs_new {
            let conn = Connection::connect(&self.config.url, ConnectionProperties::default())
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("AMQP error: {}", e)))?;

            let ch = conn
                .create_channel()
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("AMQP error: {}", e)))?;

            ch.exchange_declare(
                &self.config.exchange,
                lapin::ExchangeKind::Fanout,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("AMQP error: {}", e)))?;

            *guard = Some(ch);
        }

        let channel = guard.as_ref().unwrap();

        channel
            .basic_publish(
                &self.config.exchange,
                &self.config.routing_key,
                BasicPublishOptions::default(),
                payload.as_bytes(),
                BasicProperties::default(),
            )
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("AMQP publish error: {}", e)))?
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("AMQP confirm error: {}", e)))?;

        Ok(())
    }
}
