use pyo3::types::PyDict;
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use std::sync::{Arc, RwLock};
use tokio::runtime::{Handle, Runtime};

use super::config::BrokerConfig;
use super::Broker;

static BROADCASTER: RwLock<Option<Arc<Broadcaster>>> = RwLock::new(None);

#[pyfunction]
pub(crate) fn setup_broadcast(config: &Bound<'_, PyDict>) -> PyResult<()> {
    let broadcaster = Arc::new(Broadcaster::new(config)?);
    let mut guard = BROADCASTER
        .write()
        .map_err(|e| PyRuntimeError::new_err(format!("Lock poisoned: {}", e)))?;
    *guard = Some(broadcaster);
    Ok(())
}

#[pyfunction]
pub(crate) fn reset_broadcast() -> PyResult<()> {
    let mut guard = BROADCASTER
        .write()
        .map_err(|e| PyRuntimeError::new_err(format!("Lock poisoned: {}", e)))?;
    *guard = None;
    Ok(())
}

fn get_broadcaster() -> PyResult<Arc<Broadcaster>> {
    let guard = BROADCASTER
        .read()
        .map_err(|e| PyRuntimeError::new_err(format!("Lock poisoned: {}", e)))?;
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| PyRuntimeError::new_err("broadcast is not initialized"))
}

#[pyfunction]
pub(crate) fn broadcast(py: Python<'_>, groups: Vec<String>, message: String) -> PyResult<()> {
    let broadcaster = get_broadcaster()?;
    py.detach(|| broadcaster.send(groups, message))
}

#[pyfunction]
pub(crate) fn abroadcast(
    py: Python<'_>,
    groups: Vec<String>,
    message: String,
) -> PyResult<Bound<'_, PyAny>> {
    let broadcaster = get_broadcaster()?;
    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        broadcaster.asend(groups, message).await
    })
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

    fn send(&self, groups: Vec<String>, message: String) -> PyResult<()> {
        let payload = Self::make_payload(&groups, &message);
        if let Ok(handle) = Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(self.broker.send(payload)))
        } else {
            self.rt.block_on(self.broker.send(payload))
        }
    }

    async fn asend(&self, groups: Vec<String>, message: String) -> PyResult<()> {
        let payload = Self::make_payload(&groups, &message);
        self.broker.send(payload).await
    }
}
