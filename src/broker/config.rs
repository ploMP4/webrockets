use pyo3::prelude::*;
use pyo3::types::PyDict;

#[derive(Debug, Clone)]
pub(crate) enum BrokerConfig {
    Redis(RedisConfig),
    Amqp(AmqpConfig),
}

#[derive(Debug, Clone)]
pub(crate) struct RedisConfig {
    pub url: String,
    pub channel: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AmqpConfig {
    pub url: String,
    pub exchange: String,
    pub queue: Option<String>,
    pub routing_key: String,
}

impl BrokerConfig {
    pub(crate) fn from_py(dict: &Bound<'_, PyDict>) -> PyResult<Self> {
        let broker_type: String = dict
            .get_item("type")?
            .ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("broker 'type' is required")
            })?
            .extract()?;

        match broker_type.as_str() {
            "redis" => {
                let url: String = dict
                    .get_item("url")?
                    .map(|v| v.extract())
                    .transpose()?
                    .unwrap_or_else(|| "redis://localhost:6379".to_string());
                let channel: String = dict
                    .get_item("channel")?
                    .map(|v| v.extract())
                    .transpose()?
                    .unwrap_or_else(|| "ws_broadcast".to_string());
                Ok(BrokerConfig::Redis(RedisConfig { url, channel }))
            }
            "amqp" => {
                let url: String = dict
                    .get_item("url")?
                    .map(|v| v.extract())
                    .transpose()?
                    .unwrap_or_else(|| "amqp://localhost:5672".to_string());
                let exchange: String = dict
                    .get_item("exchange")?
                    .map(|v| v.extract())
                    .transpose()?
                    .unwrap_or_else(|| "ws_broadcast".to_string());
                let queue: Option<String> =
                    dict.get_item("queue")?.map(|v| v.extract()).transpose()?;
                let routing_key: String = dict
                    .get_item("routing_key")?
                    .map(|v| v.extract())
                    .transpose()?
                    .unwrap_or_else(|| "#".to_string());
                Ok(BrokerConfig::Amqp(AmqpConfig {
                    url,
                    exchange,
                    queue,
                    routing_key,
                }))
            }
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "unknown broker type: {}. Supported: redis, amqp",
                broker_type
            ))),
        }
    }
}
