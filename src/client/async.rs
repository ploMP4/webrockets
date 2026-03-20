use std::{sync::Arc, time::Duration};

use hyper_util::rt::TokioIo;
use tokio::sync::Mutex;

use fastwebsockets::{CloseCode, FragmentCollector, Frame, OpCode, Payload};
use hyper::upgrade::Upgraded;
use pyo3::{
    exceptions::{PyRuntimeError, PyTimeoutError},
    prelude::*,
};

use crate::client::{parse_close_payload, ws_connect, ClientConfig, ConnectionClosed};

#[pyclass]
pub(crate) struct AsyncClient {
    #[pyo3(get)]
    config: ClientConfig,
    #[pyo3(get)]
    negotiated_protocol: Option<String>,

    ws: Option<Arc<Mutex<FragmentCollector<TokioIo<Upgraded>>>>>,
    pending_url: Option<String>,
    pending_timeout: Option<u64>,
}

#[pymethods]
impl AsyncClient {
    #[new]
    #[pyo3(signature=(config=None))]
    fn __new__(config: Option<ClientConfig>) -> Self {
        AsyncClient {
            ws: None,
            pending_url: None,
            config: config.unwrap_or_default(),
            pending_timeout: None,
            negotiated_protocol: None,
        }
    }

    #[pyo3(signature=(url, timeout=None))]
    fn connect<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        url: String,
        timeout: Option<u64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let config = slf.borrow(py).config.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (ws, negotiated_protocol) = match timeout {
                Some(millis) => {
                    tokio::time::timeout(Duration::from_millis(millis), ws_connect(config, url))
                        .await
                        .map_err(|_| PyTimeoutError::new_err("connect timed out"))?
                        .map_err(|e| PyRuntimeError::new_err(format!("{e}")))?
                }
                None => ws_connect(config, url).await?,
            };
            Python::attach(|py| {
                let mut client = slf.borrow_mut(py);
                client.ws = Some(Arc::new(Mutex::new(ws)));
                client.negotiated_protocol = negotiated_protocol;
            });
            Ok(())
        })
    }

    #[pyo3(signature=(data=None))]
    fn ping<'py>(
        &self,
        py: Python<'py>,
        data: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
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

        let frame = Frame::new(true, OpCode::Ping, None, payload);
        let ws = self
            .ws
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
            .clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            ws.lock()
                .await
                .write_frame(frame)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("failed to send frame: {e}")))
        })
    }

    #[pyo3(signature=(data=None))]
    fn pong<'py>(
        &self,
        py: Python<'py>,
        data: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
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

        let frame = Frame::pong(payload);
        let ws = self
            .ws
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
            .clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            ws.lock()
                .await
                .write_frame(frame)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("failed to send frame: {e}")))
        })
    }

    fn send<'py>(&self, py: Python<'py>, data: Bound<'_, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let frame = if let Ok(s) = data.extract::<String>() {
            Frame::text(Payload::Owned(s.into_bytes()))
        } else if let Ok(b) = data.extract::<&[u8]>() {
            Frame::binary(Payload::Owned(b.to_vec()))
        } else {
            return Err(PyRuntimeError::new_err("data must be str or bytes"));
        };

        let ws = self
            .ws
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
            .clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            ws.lock()
                .await
                .write_frame(frame)
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("failed to send frame: {e}")))
        })
    }

    #[pyo3(signature=(timeout=None))]
    fn recv<'py>(&self, py: Python<'py>, timeout: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let ws = self
            .ws
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
            .clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let frame = match timeout {
                Some(millis) => tokio::time::timeout(
                    Duration::from_millis(millis),
                    ws.lock().await.read_frame(),
                )
                .await
                .map_err(|_| PyTimeoutError::new_err("recv timed out"))?
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))?,
                None => ws
                    .lock()
                    .await
                    .read_frame()
                    .await
                    .map_err(|e| PyRuntimeError::new_err(format!("{e}")))?,
            };

            Python::attach(|py| match frame.opcode {
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
            })
        })
    }

    #[pyo3(signature = (code = CloseCode::Normal.into(), reason = "".to_string()))]
    fn close<'py>(
        &self,
        py: Python<'py>,
        code: u16,
        reason: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let ws = self
            .ws
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
            .clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            ws.lock()
                .await
                .write_frame(Frame::close(code, reason.as_bytes()))
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
        })
    }

    fn __aenter__<'py>(slf: Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let config = slf.borrow().config.clone();
        let url = slf
            .borrow()
            .pending_url
            .clone()
            .ok_or_else(|| PyRuntimeError::new_err("no url was provided"))?;

        let timeout = slf.borrow().pending_timeout;

        let slf = slf.unbind();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (ws, negotiated_protocol) = match timeout {
                Some(millis) => {
                    tokio::time::timeout(Duration::from_millis(millis), ws_connect(config, url))
                        .await
                        .map_err(|_| PyTimeoutError::new_err("connect timed out"))?
                        .map_err(|e| PyRuntimeError::new_err(format!("{e}")))?
                }
                None => ws_connect(config, url).await?,
            };
            Python::attach(|py| {
                let mut client = slf.borrow_mut(py);
                client.ws = Some(Arc::new(Mutex::new(ws)));
                client.negotiated_protocol = negotiated_protocol;
            });
            Ok(slf)
        })
    }

    fn __aexit__<'py>(
        &self,
        py: Python<'py>,
        exc_type: Option<Py<PyAny>>,
        _exc: Py<PyAny>,
        _traceback: Py<PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let code = if exc_type.is_none() {
            CloseCode::Normal.into()
        } else {
            CloseCode::Error.into()
        };

        let ws = self.ws.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Ignore errors when closing - connection may already be closed
            if let Some(ws) = ws {
                let _ = ws
                    .lock()
                    .await
                    .write_frame(Frame::close(code, b""[..].into()))
                    .await;
            }
            Ok(())
        })
    }
}

#[pyfunction]
#[pyo3(signature=(url, config=None, timeout=None))]
pub(crate) fn aconnect(
    url: String,
    config: Option<ClientConfig>,
    timeout: Option<u64>,
) -> AsyncClient {
    AsyncClient {
        ws: None,
        pending_url: Some(url),
        config: config.unwrap_or_default(),
        pending_timeout: timeout,
        negotiated_protocol: None,
    }
}
