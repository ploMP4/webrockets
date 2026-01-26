use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Empty;
use hyper_util::rt::TokioIo;
use tokio::sync::Mutex;

use fastwebsockets::{CloseCode, FragmentCollector, Frame, OpCode, Payload};
use hyper::{
    header::{CONNECTION, UPGRADE},
    upgrade::Upgraded,
    Request, Uri,
};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::net::TcpStream;

use crate::client::SpawnExecutor;

async fn do_connect(slf: &Py<AsyncClient>, url: String) -> PyResult<()> {
    let uri: Uri = url
        .parse()
        .map_err(|e| PyRuntimeError::new_err(format!("invalid URL: {e}")))?;

    let host = uri
        .host()
        .ok_or_else(|| PyRuntimeError::new_err("URL missing host"))?;

    let port = uri.port_u16().unwrap_or(80);
    let addr = format!("{host}:{port}");

    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| PyRuntimeError::new_err(format!("unable to connect to {addr}: {e}")))?;

    let req = Request::builder()
        .method("GET")
        .uri(&uri)
        .header("Host", &addr)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(
            "Sec-WebSocket-Key",
            fastwebsockets::handshake::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .body(Empty::<Bytes>::new())
        .map_err(|e| PyRuntimeError::new_err(format!("unable to construct request: {e}")))?;

    let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
        .await
        .map_err(|e| PyRuntimeError::new_err(format!("error on client handshake: {e}")))?;

    Python::attach(|py| {
        slf.borrow_mut(py).ws = Some(Arc::new(Mutex::new(FragmentCollector::new(ws))));
        Ok(())
    })
}

#[pyclass]
pub(super) struct AsyncClient {
    ws: Option<Arc<Mutex<FragmentCollector<TokioIo<Upgraded>>>>>,
    pending_url: Option<String>,
}

#[pymethods]
impl AsyncClient {
    #[new]
    fn __new__() -> Self {
        AsyncClient {
            ws: None,
            pending_url: None,
        }
    }

    fn connect<'py>(slf: Py<Self>, py: Python<'py>, url: String) -> PyResult<Bound<'py, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move { do_connect(&slf, url).await })
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

    fn recv<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ws = self
            .ws
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
            .clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let frame = ws
                .lock()
                .await
                .read_frame()
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))?;

            Python::attach(|py| match frame.opcode {
                OpCode::Text => Ok(std::str::from_utf8(&frame.payload)
                    .map_err(|e| PyRuntimeError::new_err(format!("invalid UTF-8: {e}")))?
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()),
                OpCode::Binary => Ok(frame.payload.into_pyobject(py)?.into_any().unbind()),
                OpCode::Close => Err(PyRuntimeError::new_err("connection closed")),
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
                .write_frame(Frame::close(code, reason.as_bytes().into()))
                .await
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
        })
    }

    fn __aenter__<'py>(slf: Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let url = slf
            .borrow()
            .pending_url
            .clone()
            .ok_or_else(|| PyRuntimeError::new_err("no url was provided"))?;

        let slf = slf.unbind();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            do_connect(&slf, url).await?;
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
        if exc_type.is_none() {
            self.close(py, CloseCode::Normal.into(), "".to_string())
        } else {
            self.close(py, CloseCode::Error.into(), "".to_string())
        }
    }
}

#[pyfunction]
pub(super) fn aconnect(url: String) -> AsyncClient {
    AsyncClient {
        ws: None,
        pending_url: Some(url),
    }
}
