use std::collections::HashMap;
use std::sync::Arc;

use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::sync::mpsc;

use crate::{Message, ASYNCIO_SLEEP};

#[pyclass(subclass)]
pub struct BaseConnection {
    #[pyo3(get)]
    pub path: String,
    #[pyo3(get)]
    pub query_string: String,
    #[pyo3(get)]
    pub headers: HashMap<String, String>,
    #[pyo3(get)]
    pub cookies: HashMap<String, String>,
    #[pyo3(get)]
    pub user: Option<Py<PyAny>>,
}

impl BaseConnection {
    pub fn new(
        path: String,
        query_string: String,
        headers: HashMap<String, String>,
        cookies: HashMap<String, String>,
    ) -> Self {
        Self {
            path,
            query_string,
            headers,
            cookies,
            user: None,
        }
    }
}

#[pymethods]
impl BaseConnection {
    #[new]
    fn __new__(
        path: String,
        query_string: String,
        headers: HashMap<String, String>,
        cookies: HashMap<String, String>,
    ) -> Self {
        Self::new(path, query_string, headers, cookies)
    }

    fn get_cookie(&self, name: String) -> Option<&String> {
        self.cookies.get(&name)
    }

    fn get_header(&self, name: String) -> Option<&String> {
        self.headers.get(&name)
    }
}

#[pyclass(extends=BaseConnection)]
pub struct IncomingConnection;

impl IncomingConnection {
    pub fn upgrade(incoming: &Py<IncomingConnection>, py: Python<'_>) -> PyResult<Py<Connection>> {
        let borrowed = incoming.borrow(py);
        let base = borrowed.as_super();

        Py::new(
            py,
            (
                Connection::new(),
                BaseConnection {
                    path: base.path.clone(),
                    query_string: base.query_string.clone(),
                    headers: base.headers.clone(),
                    cookies: base.cookies.clone(),
                    user: base.user.as_ref().map(|u| u.clone_ref(py)),
                },
            ),
        )
    }
}

#[pyclass(extends=BaseConnection)]
pub(crate) struct Connection {
    pub(crate) sender: Option<Arc<mpsc::Sender<Arc<Message>>>>,
}

impl Connection {
    pub fn new() -> Self {
        Self { sender: None }
    }
}

#[pymethods]
impl Connection {
    #[new]
    fn __new__(
        path: String,
        query_string: String,
        headers: HashMap<String, String>,
        cookies: HashMap<String, String>,
    ) -> (Self, BaseConnection) {
        (
            Self::new(),
            BaseConnection::new(path, query_string, headers, cookies),
        )
    }

    fn send(&self, py: Python<'_>, msg: String) -> PyResult<()> {
        let tx = self
            .sender
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        let message = Arc::new(Message::Text(msg.into()));

        match tx.try_send(Arc::clone(&message)) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                let tx = Arc::clone(tx);
                py.detach(|| {
                    tokio::spawn(async move {
                        let _ = tx.send(message).await;
                    });
                });
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(()),
        }
    }

    fn asend<'py>(&self, py: Python<'py>, msg: String) -> PyResult<Bound<'py, PyAny>> {
        let tx = self
            .sender
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        let message = Arc::new(Message::Text(msg.into()));

        match tx.try_send(Arc::clone(&message)) {
            Ok(()) => {
                let sleep = ASYNCIO_SLEEP
                    .get()
                    .ok_or_else(|| PyRuntimeError::new_err("asyncio.sleep not initialized"))?;
                Ok(sleep.call1(py, (0,))?.into_bound(py))
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                let tx = Arc::clone(tx);
                pyo3_async_runtimes::tokio::future_into_py(py, async move {
                    let _ = tx.send(message).await;
                    Ok(())
                })
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                let sleep = ASYNCIO_SLEEP
                    .get()
                    .ok_or_else(|| PyRuntimeError::new_err("asyncio.sleep not initialized"))?;
                Ok(sleep.call1(py, (0,))?.into_bound(py))
            }
        }
    }
}
