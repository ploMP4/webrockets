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

use crate::client::{SpawnExecutor, RUNTIME};

#[pyclass]
struct AsyncClient {
    ws: Option<Mutex<FragmentCollector<TokioIo<Upgraded>>>>,
}

#[pymethods]
impl AsyncClient {
    #[new]
    fn __new__() -> Self {
        AsyncClient { ws: None }
    }
}

// #[pyfunction]
// fn connect(py: Python<'_>, url: String) -> PyResult<AsyncClient> {
//     let mut client = AsyncClient { ws: None };
//     client.connect(py, url)?;
//     Ok(client)
// }

#[pymodule(name = "async")]
pub(super) mod async_client {
    // #[pymodule_export]
    // use super::connect;
    #[pymodule_export]
    use super::AsyncClient;
}
