use std::collections::HashMap;
use std::sync::Arc;

use axum::http::{HeaderMap, Uri};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::sync::mpsc;

use crate::{Message, ASYNCIO_SLEEP};

#[pyclass(subclass, get_all)]
pub(crate) struct BaseConnection {
    path: String,
    query_string: String,
    headers: HashMap<String, String>,
    cookies: HashMap<String, String>,
    #[pyo3(set)]
    pub(crate) user: Option<Py<PyAny>>,
}

impl BaseConnection {
    pub(crate) fn new(
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
pub(crate) struct IncomingConnection;

impl IncomingConnection {
    pub(crate) fn py_new(uri: &Uri, header_map: &HeaderMap) -> Py<Self> {
        let path = uri.path().to_owned();
        let query_string = uri.query().unwrap_or_default().to_owned();

        let mut headers = HashMap::with_capacity(header_map.len());
        for (name, value) in header_map.iter() {
            if let Ok(v) = value.to_str() {
                headers.insert(name.as_str().to_lowercase(), v.to_owned());
            }
        }

        let mut cookies = HashMap::new();
        if let Some(cookie_header) = header_map.get("cookie") {
            if let Ok(cookie_str) = cookie_header.to_str() {
                cookies.reserve(4);
                for cookie in cookie_str.split(';') {
                    let cookie = cookie.trim();
                    if let Some((name, value)) = cookie.split_once('=') {
                        cookies.insert(name.trim().to_owned(), value.trim().to_owned());
                    }
                }
            }
        }

        Python::attach(|py| -> Py<IncomingConnection> {
            Py::new(
                py,
                (
                    IncomingConnection,
                    BaseConnection::new(path, query_string, headers, cookies),
                ),
            )
            .expect("Unable to create connection")
        })
    }

    pub(crate) fn upgrade(
        incoming: &Py<IncomingConnection>,
        py: Python<'_>,
    ) -> PyResult<Py<Connection>> {
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
    pub(crate) fn new() -> Self {
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
