use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::PyResult;
use redis::AsyncCommands;
use tokio::sync::{watch, Mutex};
use tokio::time::sleep;

use super::config::RedisConfig;
use super::BrokerMessage;
use crate::broker::Broker;
use crate::channel_store::ChannelStore;

pub(crate) struct Redis {
    config: RedisConfig,
    conn: Mutex<Option<redis::aio::MultiplexedConnection>>,
}

impl Redis {
    pub(crate) fn new(config: &RedisConfig) -> Self {
        Self {
            config: config.clone(),
            conn: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Broker for Redis {
    async fn listen(&self, channels: Arc<ChannelStore>, mut shutdown: watch::Receiver<bool>) {
        shutdown.mark_unchanged();
        let mut backoff = Duration::from_millis(100);
        const MAX_BACKOFF: Duration = Duration::from_secs(30);

        loop {
            let client = match redis::Client::open(self.config.url.as_str()) {
                Ok(client) => client,
                Err(e) => {
                    log::warn!(
                        "Redis client creation failed: {}, retrying in {:?}",
                        e,
                        backoff
                    );
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, MAX_BACKOFF);
                    continue;
                }
            };

            let pubsub = match client.get_async_pubsub().await {
                Ok(ps) => ps,
                Err(e) => {
                    log::warn!("Redis connection failed: {}, retrying in {:?}", e, backoff);
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, MAX_BACKOFF);
                    continue;
                }
            };

            let mut pubsub = pubsub;
            if let Err(e) = pubsub.subscribe(&self.config.channel).await {
                log::warn!("Redis subscribe failed: {}, retrying in {:?}", e, backoff);
                sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, MAX_BACKOFF);
                continue;
            }

            log::info!(
                "Connected to Redis, subscribed to channel '{}'",
                self.config.channel
            );
            backoff = Duration::from_millis(100);

            let mut stream = pubsub.into_on_message();

            loop {
                tokio::select! {
                    _ = shutdown.changed() => {
                        log::info!("Redis listener shutting down");
                        return;
                    }
                    msg = stream.next() => {
                        match msg {
                            Some(msg) => {
                                let payload: String = match msg.get_payload() {
                                    Ok(p) => p,
                                    Err(e) => {
                                        log::error!("Redis payload error: {}", e);
                                        continue;
                                    }
                                };

                                match serde_json::from_str::<BrokerMessage>(&payload) {
                                    Ok(broker_msg) => {
                                        broker_msg.dispatch(&channels);
                                    }
                                    Err(e) => {
                                        log::error!("Invalid broker message format: {} - payload: {}", e, payload);
                                    }
                                }
                            }
                            None => {
                                log::warn!("Redis stream ended, reconnecting...");
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn send(&self, payload: String) -> PyResult<()> {
        let mut guard = self.conn.lock().await;

        let conn = if let Some(ref mut conn) = *guard {
            conn
        } else {
            let client = redis::Client::open(self.config.url.clone())
                .map_err(|e| PyRuntimeError::new_err(format!("Redis error: {}", e)))?;
            let new_conn = client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("Redis error: {}", e)))?;
            *guard = Some(new_conn);
            guard.as_mut().unwrap()
        };

        conn.publish::<_, _, ()>(&self.config.channel, &payload)
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("Redis publish error: {}", e)))?;

        Ok(())
    }
}
