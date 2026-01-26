pub(crate) mod broadcaster;
pub(crate) mod config;

pub(crate) mod amqp;
pub(crate) mod redis;

use async_trait::async_trait;
pub(crate) use broadcaster::{abroadcast, broadcast, reset_broadcast, setup_broadcast};
use pyo3::PyResult;

use std::sync::Arc;
use tokio::sync::watch;

use crate::channel_store::ChannelStore;
use crate::Message;

#[async_trait]
pub(crate) trait Broker {
    async fn listen(&self, channels: Arc<ChannelStore>, shutdown: watch::Receiver<bool>);
    async fn send(&self, payload: String) -> PyResult<()>;
}

pub(crate) fn get_broker(config: &config::BrokerConfig) -> Box<dyn Broker + Send + Sync> {
    match config {
        config::BrokerConfig::Redis(cfg) => Box::new(redis::Redis::new(cfg)),
        config::BrokerConfig::Amqp(cfg) => Box::new(amqp::Amqp::new(cfg)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct BrokerMessage {
    pub(crate) groups: Vec<String>,
    pub(crate) message: String,
}

impl BrokerMessage {
    pub(crate) fn dispatch(self, channels: &ChannelStore) {
        channels.broadcast(
            &self.groups,
            Arc::new(Message::Text(self.message.into())),
            None,
        );
    }
}

pub(crate) async fn broker_listener(
    config: config::BrokerConfig,
    channels: Arc<ChannelStore>,
    shutdown: watch::Receiver<bool>,
) {
    get_broker(&config).listen(channels, shutdown).await;
}
