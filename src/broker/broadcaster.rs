use pyo3::types::PyDict;
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use std::sync::OnceLock;
use tokio::runtime::Runtime;

use super::config::BrokerConfig;
use super::Broker;

static BROADCASTER: OnceLock<Broadcaster> = OnceLock::new();

#[pyfunction]
pub(crate) fn setup_broadcast(config: &Bound<'_, PyDict>) -> PyResult<()> {
    let _ =
        BROADCASTER
            .set(Broadcaster::new(config).map_err(|e| {
                PyRuntimeError::new_err(format!("Failed to create runtime: {}", e))
            })?);
    Ok(())
}

#[pyfunction]
pub(crate) fn broadcast(py: Python<'_>, groups: Vec<String>, message: String) -> PyResult<()> {
    let broadcaster = BROADCASTER
        .get()
        .ok_or_else(|| PyRuntimeError::new_err("broadcast is not initialized"))?;

    py.detach(|| broadcaster.send(groups, message))
}

#[pyfunction]
pub(crate) fn abroadcast(
    py: Python<'_>,
    groups: Vec<String>,
    message: String,
) -> PyResult<Bound<'_, PyAny>> {
    let broadcaster = BROADCASTER
        .get()
        .ok_or_else(|| PyRuntimeError::new_err("broadcast is not initialized"))?;

    pyo3_async_runtimes::tokio::future_into_py(py, broadcaster.asend(groups, message))
}

#[pyclass]
pub struct Broadcaster {
    broker: Box<dyn Broker + Send + Sync>,
    rt: Runtime,
}

impl Broadcaster {
    fn new(config: &Bound<'_, PyDict>) -> PyResult<Self> {
        let config = BrokerConfig::from_py(config)?;
        let rt = Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

        let broker = super::get_broker(&config);

        Ok(Self { broker, rt })
    }

    fn make_payload(groups: &[String], message: &str) -> String {
        serde_json::json!({
            "groups": groups,
            "message": message
        })
        .to_string()
    }
}

impl Broadcaster {
    fn send(&self, groups: Vec<String>, message: String) -> PyResult<()> {
        let payload = Self::make_payload(&groups, &message);
        self.rt.block_on(self.broker.send(payload))
    }

    async fn asend(&self, groups: Vec<String>, message: String) -> PyResult<()> {
        let payload = Self::make_payload(&groups, &message);
        self.broker.send(payload).await
    }
}
