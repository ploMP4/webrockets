use bytes::Bytes;
use http_body_util::Empty;
use std::{future::Future, sync::LazyLock};

use fastwebsockets::FragmentCollector;
use hyper::{
    header::{CONNECTION, UPGRADE},
    upgrade::Upgraded,
    Request, Uri,
};
use hyper_util::rt::TokioIo;
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::{net::TcpStream, runtime::Runtime};

use crate::client::config::ClientConfig;

mod r#async;
mod config;
mod sync;

pub(crate) static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("unable to start client runtime"));

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

pub(crate) async fn ws_connect(
    config: ClientConfig,
    url: String,
) -> PyResult<FragmentCollector<TokioIo<Upgraded>>> {
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

    if let Some(extra_headers) = &config.extra_headers {
        for (key, value) in extra_headers.iter() {
            req_builder = req_builder.header(key, value);
        }
    }

    let req = req_builder
        .body(Empty::<Bytes>::new())
        .map_err(|e| PyRuntimeError::new_err(format!("unable to construct request: {e}")))?;

    let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
        .await
        .map_err(|e| PyRuntimeError::new_err(format!("error on client handshake: {e}")))?;

    Ok(FragmentCollector::new(ws))
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
}
