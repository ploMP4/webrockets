use bytes::Bytes;
use http_body_util::Empty;
use rustls_platform_verifier::ConfigVerifierExt;
use std::{
    future::Future,
    sync::{Arc, LazyLock},
};
use tokio_rustls::TlsConnector;
use tokio_util::either::Either;

use fastwebsockets::{CloseCode, FragmentCollector, WebSocketError};
use hyper::{
    header::{CONNECTION, UPGRADE},
    upgrade::Upgraded,
    Request, Uri,
};
use hyper_util::rt::TokioIo;
use pyo3::{
    exceptions::{PyException, PyRuntimeError},
    prelude::*,
    ToPyErr,
};
use tokio::{net::TcpStream, runtime::Runtime};

use crate::client::config::ClientConfig;

mod r#async;
mod config;
mod sync;

pub(crate) static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("unable to start client runtime"));

#[pyclass(extends=PyException)]
pub struct ConnectionClosed {
    #[pyo3(get)]
    pub code: u16,
    #[pyo3(get)]
    pub reason: String,
}

#[pymethods]
impl ConnectionClosed {
    #[new]
    #[pyo3(signature = (code = 1000, reason = "".to_string()))]
    fn new(code: u16, reason: String) -> Self {
        ConnectionClosed { code, reason }
    }

    fn __str__(&self) -> String {
        if self.reason.is_empty() {
            format!("ConnectionClosed(code={})", self.code)
        } else {
            format!(
                "ConnectionClosed(code={}, reason={:?})",
                self.code, self.reason
            )
        }
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}

impl ToPyErr for ConnectionClosed {}

#[pyclass(extends=PyException)]
pub struct InvalidStatusCode {
    #[pyo3(get)]
    pub status_code: u16,
}

#[pymethods]
impl InvalidStatusCode {
    #[new]
    fn new(status_code: u16) -> Self {
        InvalidStatusCode { status_code }
    }

    fn __str__(&self) -> String {
        format!("InvalidStatusCode(status_code={})", self.status_code)
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}

impl ToPyErr for InvalidStatusCode {}

/// Parse a WebSocket close frame payload into (code, reason).
/// Close frame payload format: 2-byte big-endian code followed by UTF-8 reason.
pub(crate) fn parse_close_payload(payload: &[u8]) -> (u16, String) {
    if payload.len() >= 2 {
        let code = u16::from_be_bytes([payload[0], payload[1]]);
        let reason = std::str::from_utf8(&payload[2..]).unwrap_or("").to_string();
        (code, reason)
    } else {
        (CloseCode::Status.into(), String::new())
    }
}

pub(crate) struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

fn tls_connector() -> Result<TlsConnector, rustls::Error> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let config = tokio_rustls::rustls::ClientConfig::with_platform_verifier()?;
    Ok(TlsConnector::from(Arc::new(config)))
}

pub(crate) async fn ws_connect(
    config: ClientConfig,
    url: String,
) -> PyResult<(FragmentCollector<TokioIo<Upgraded>>, Option<String>)> {
    let uri: Uri = url
        .parse()
        .map_err(|e| PyRuntimeError::new_err(format!("invalid URL: {e}")))?;

    let host = uri
        .host()
        .ok_or_else(|| PyRuntimeError::new_err("URL missing host"))?;

    let default_port = match uri.scheme_str() {
        Some("wss") => 443,
        _ => 80,
    };
    let port = uri.port_u16().unwrap_or(default_port);
    let addr = format!("{host}:{port}");

    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| PyRuntimeError::new_err(format!("unable to connect to {addr}: {e}")))?;

    let stream = if config.verify_ssl {
        let tls_connector = tls_connector().unwrap();
        let domain = tokio_rustls::rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid dnsname")
        })?;
        Either::Left(tls_connector.connect(domain, stream).await?)
    } else {
        Either::Right(stream)
    };

    let mut req_builder = Request::builder()
        .method("GET")
        .uri(&uri)
        .header("Host", &addr)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(
            "Sec-WebSocket-Key",
            fastwebsockets::handshake::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13");

    if let Some(protocols) = &config.subprotocols {
        req_builder = req_builder.header("Sec-WebSocket-Protocol", protocols.join(", "));
    }

    if let Some(extra_headers) = &config.extra_headers {
        for (key, value) in extra_headers.iter() {
            req_builder = req_builder.header(key, value);
        }
    }

    let req = req_builder
        .body(Empty::<Bytes>::new())
        .map_err(|e| PyRuntimeError::new_err(format!("unable to construct request: {e}")))?;

    let (mut ws, response) = fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
        .await
        .map_err(|e| match e {
            WebSocketError::InvalidStatusCode(status_code) => {
                PyErr::new::<InvalidStatusCode, _>((status_code,))
            }
            _ => PyRuntimeError::new_err(format!("error on client handshake: {e}")),
        })?;

    let negotiated_protocol = response
        .headers()
        .get("Sec-WebSocket-Protocol")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());

    if let Some(size) = config.max_message_size {
        ws.set_max_message_size(size);
    }

    Ok((FragmentCollector::new(ws), negotiated_protocol))
}

#[pymodule]
pub(crate) mod client {
    #[pymodule_export]
    use super::config::ClientConfig;
    #[pymodule_export]
    use super::r#async::aconnect;
    #[pymodule_export]
    use super::r#async::AsyncClient;
    #[pymodule_export]
    use super::sync::connect;
    #[pymodule_export]
    use super::sync::Client;
    #[pymodule_export]
    use super::ConnectionClosed;
    #[pymodule_export]
    use super::InvalidStatusCode;
}
