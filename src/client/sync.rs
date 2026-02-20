use hyper_util::rt::TokioIo;
use std::{sync::Mutex, time::Duration};

use fastwebsockets::{CloseCode, FragmentCollector, Frame, OpCode, Payload};
use hyper::upgrade::Upgraded;
use pyo3::{
    exceptions::{PyRuntimeError, PyTimeoutError},
    prelude::*,
};

use crate::client::{
    config::ClientConfig, parse_close_payload, ws_connect, ConnectionClosed, RUNTIME,
};

#[pyclass]
pub(crate) struct Client {
    #[pyo3(get)]
    config: ClientConfig,

    ws: Option<Mutex<FragmentCollector<TokioIo<Upgraded>>>>,
}

#[pymethods]
impl Client {
    #[new]
    #[pyo3(signature=(config=None))]
    fn __new__(config: Option<ClientConfig>) -> Self {
        Client {
            ws: None,
            config: config.unwrap_or_default(),
        }
    }

    #[pyo3(signature=(url, timeout=None))]
    fn connect(&mut self, py: Python<'_>, url: String, timeout: Option<u64>) -> PyResult<()> {
        py.detach(|| {
            let ws = RUNTIME.block_on(async {
                match timeout {
                    Some(millis) => tokio::time::timeout(
                        Duration::from_millis(millis),
                        ws_connect(self.config.clone(), url),
                    )
                    .await
                    .map_err(|_| PyTimeoutError::new_err("connect timed out"))?
                    .map_err(|e| PyRuntimeError::new_err(format!("{e}"))),
                    None => ws_connect(self.config.clone(), url).await,
                }
            })?;
            self.ws = Some(Mutex::new(ws));
            Ok(())
        })
    }

    #[pyo3(signature=(data=None))]
    fn ping(&self, py: Python<'_>, data: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        let payload = match data {
            Some(data) => {
                if let Ok(s) = data.extract::<String>() {
                    Payload::Owned(s.into_bytes())
                } else if let Ok(b) = data.extract::<&[u8]>() {
                    Payload::Owned(b.to_vec())
                } else {
                    return Err(PyRuntimeError::new_err("data must be str or bytes"));
                }
            }
            None => Payload::Owned(Vec::new()),
        };

        py.detach(|| {
            let frame = Frame::new(true, OpCode::Ping, None, payload);
            let mut guard = self
                .ws
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

            RUNTIME
                .block_on(guard.write_frame(frame))
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
        })
    }

    #[pyo3(signature=(data=None))]
    fn pong(&self, py: Python<'_>, data: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        let payload = match data {
            Some(data) => {
                if let Ok(s) = data.extract::<String>() {
                    Payload::Owned(s.into_bytes())
                } else if let Ok(b) = data.extract::<&[u8]>() {
                    Payload::Owned(b.to_vec())
                } else {
                    return Err(PyRuntimeError::new_err("data must be str or bytes"));
                }
            }
            None => Payload::Owned(Vec::new()),
        };

        py.detach(|| {
            let frame = Frame::pong(payload);
            let mut guard = self
                .ws
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

            RUNTIME
                .block_on(guard.write_frame(frame))
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
        })
    }

    fn send(&self, py: Python<'_>, data: Bound<'_, PyAny>) -> PyResult<()> {
        let frame = if let Ok(s) = data.extract::<String>() {
            Frame::text(Payload::Owned(s.into_bytes()))
        } else if let Ok(b) = data.extract::<&[u8]>() {
            Frame::binary(Payload::Owned(b.to_vec()))
        } else {
            return Err(PyRuntimeError::new_err("data must be str or bytes"));
        };

        py.detach(|| {
            let mut guard = self
                .ws
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

            RUNTIME
                .block_on(guard.write_frame(frame))
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
        })
    }

    #[pyo3(signature=(timeout=None))]
    fn recv(&self, py: Python<'_>, timeout: Option<u64>) -> PyResult<Py<PyAny>> {
        let frame = py.detach(|| {
            let mut guard = self
                .ws
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

            RUNTIME.block_on(async {
                match timeout {
                    Some(millis) => {
                        tokio::time::timeout(Duration::from_millis(millis), guard.read_frame())
                            .await
                            .map_err(|_| PyTimeoutError::new_err("recv timed out"))?
                            .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
                    }
                    None => guard
                        .read_frame()
                        .await
                        .map_err(|e| PyRuntimeError::new_err(format!("{e}"))),
                }
            })
        })?;

        match frame.opcode {
            OpCode::Text => Ok(std::str::from_utf8(&frame.payload)
                .map_err(|e| PyRuntimeError::new_err(format!("invalid UTF-8: {e}")))?
                .into_pyobject(py)?
                .into_any()
                .unbind()),
            OpCode::Binary => Ok(frame.payload.into_pyobject(py)?.into_any().unbind()),
            OpCode::Close => {
                let (code, reason) = parse_close_payload(&frame.payload);
                Err(PyErr::new::<ConnectionClosed, _>((code, reason)))
            }
            _ => Err(PyRuntimeError::new_err(format!(
                "unexpected opcode: {:?}",
                frame.opcode
            ))),
        }
    }

    #[pyo3(signature = (code = CloseCode::Normal.into(), reason = ""))]
    fn close(&self, py: Python<'_>, code: u16, reason: &str) -> PyResult<()> {
        py.detach(|| {
            let mut guard = self
                .ws
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

            RUNTIME
                .block_on(guard.write_frame(Frame::close(code, reason.as_bytes())))
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
        })
    }

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        exc_type: Option<Py<PyAny>>,
        _exc: Py<PyAny>,
        _traceback: Py<PyAny>,
    ) -> PyResult<()> {
        let code = if exc_type.is_none() {
            CloseCode::Normal.into()
        } else {
            CloseCode::Error.into()
        };
        // Ignore errors when closing - connection may already be closed
        let _ = self.close(py, code, "");
        Ok(())
    }
}

#[pyfunction]
#[pyo3(signature=(url, config=None, timeout=None))]
pub(crate) fn connect(
    py: Python<'_>,
    url: String,
    config: Option<ClientConfig>,
    timeout: Option<u64>,
) -> PyResult<Client> {
    let mut client = Client {
        ws: None,
        config: config.unwrap_or_default(),
    };
    client.connect(py, url, timeout)?;
    Ok(client)
}
