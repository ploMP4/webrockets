use bytes::Bytes;
use http_body_util::Empty;
use hyper_util::rt::TokioIo;
use std::sync::Mutex;

use fastwebsockets::{CloseCode, FragmentCollector, Frame, OpCode, Payload};
use hyper::{
    header::{CONNECTION, UPGRADE},
    upgrade::Upgraded,
    Request, Uri,
};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::net::TcpStream;

use crate::client::{SpawnExecutor, RUNTIME};

#[pyclass]
pub(super) struct Client {
    ws: Option<Mutex<FragmentCollector<TokioIo<Upgraded>>>>,
}

#[pymethods]
impl Client {
    #[new]
    fn __new__() -> Self {
        Client { ws: None }
    }

    fn connect(&mut self, py: Python<'_>, url: String) -> PyResult<()> {
        py.detach(|| {
            RUNTIME.block_on(async {
                let uri: Uri = url
                    .parse()
                    .map_err(|e| PyRuntimeError::new_err(format!("invalid URL: {e}")))?;

                let host = uri
                    .host()
                    .ok_or_else(|| PyRuntimeError::new_err("URL missing host"))?;

                let port = uri.port_u16().unwrap_or(80);
                let addr = format!("{host}:{port}");

                let stream = TcpStream::connect(&addr).await.map_err(|e| {
                    PyRuntimeError::new_err(format!("unable to connect to {addr}: {e}"))
                })?;

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
                    .map_err(|e| {
                        PyRuntimeError::new_err(format!("unable to construct request: {e}"))
                    })?;

                let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
                    .await
                    .map_err(|e| {
                        PyRuntimeError::new_err(format!("error on client handshake: {e}"))
                    })?;

                self.ws = Some(Mutex::new(FragmentCollector::new(ws)));
                Ok(())
            })
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

    fn recv(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let frame = py.detach(|| {
            let mut guard = self
                .ws
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err("no websocket connection"))?
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

            RUNTIME
                .block_on(guard.read_frame())
                .map_err(|e| PyRuntimeError::new_err(format!("{e}")))
        })?;

        match frame.opcode {
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
        }
    }

    #[pyo3(signature = (code = CloseCode::Normal.into(), reason = ""))]
    fn close(&self, py: Python<'_>, code: u16, reason: &str) -> PyResult<()> {
        py.detach(|| {
            let mut guard = self
                .ws
                .as_ref()
                .ok_or_else(|| PyRuntimeError::new_err(format!("no websocket connection")))?
                .lock()
                .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

            RUNTIME
                .block_on(guard.write_frame(Frame::close(code, reason.as_bytes().into())))
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
        if exc_type.is_none() {
            self.close(py, CloseCode::Normal.into(), "")
        } else {
            self.close(py, CloseCode::Error.into(), "")
        }
    }
}

#[pyfunction]
pub(super) fn connect(py: Python<'_>, url: String) -> PyResult<Client> {
    let mut client = Client { ws: None };
    client.connect(py, url)?;
    Ok(client)
}
