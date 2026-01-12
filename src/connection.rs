use std::collections::HashMap;
use std::sync::Arc;

use axum::http::{HeaderMap, Uri};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::sync::mpsc;

use crate::{channel_store::ChannelStore, Message, ASYNC_NOOP};

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
    pub(crate) channels: Option<Arc<ChannelStore>>,
    pub(crate) id: Option<u64>,
}

impl Connection {
    pub(crate) fn new() -> Self {
        Self {
            sender: None,
            channels: None,
            id: None,
        }
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

    fn send(&self, py: Python<'_>, msg: Py<PyAny>) -> PyResult<()> {
        let tx = self
            .sender
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        let bound = msg.bind(py);
        let message = Arc::new(Message::try_from(bound)?);

        match tx.try_send(Arc::clone(&message)) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                let tx = Arc::clone(tx);
                tokio::spawn(async move {
                    let _ = tx.send(message).await;
                });
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(()),
        }
    }

    fn asend<'py>(&self, py: Python<'py>, msg: Py<PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let tx = self
            .sender
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        let bound = msg.bind(py);
        let message = Arc::new(Message::try_from(bound)?);

        match tx.try_send(Arc::clone(&message)) {
            Ok(()) => noop_coroutine(py),
            Err(mpsc::error::TrySendError::Full(_)) => {
                let tx = Arc::clone(tx);
                pyo3_async_runtimes::tokio::future_into_py(py, async move {
                    let _ = tx.send(message).await;
                    Ok(())
                })
            }
            Err(mpsc::error::TrySendError::Closed(_)) => noop_coroutine(py),
        }
    }

    #[pyo3(signature = (code=1000, reason=""))]
    fn close(&self, code: u16, reason: &str) -> PyResult<()> {
        let tx = self
            .sender
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        let message = Arc::new(Message::Close(code, reason.as_bytes().into()));

        match tx.try_send(Arc::clone(&message)) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                let tx = Arc::clone(tx);
                tokio::spawn(async move {
                    let _ = tx.send(message).await;
                });
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(()),
        }
    }

    #[pyo3(signature = (code=1000, reason=""))]
    fn aclose<'py>(&self, py: Python<'py>, code: u16, reason: &str) -> PyResult<Bound<'py, PyAny>> {
        let tx = self
            .sender
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        let message = Arc::new(Message::Close(code, reason.as_bytes().into()));

        match tx.try_send(Arc::clone(&message)) {
            Ok(()) => noop_coroutine(py),
            Err(mpsc::error::TrySendError::Full(_)) => {
                let tx = Arc::clone(tx);
                pyo3_async_runtimes::tokio::future_into_py(py, async move {
                    let _ = tx.send(message).await;
                    Ok(())
                })
            }
            Err(mpsc::error::TrySendError::Closed(_)) => noop_coroutine(py),
        }
    }

    #[pyo3(signature = (groups, msg, exclude_self=false))]
    fn broadcast(
        &self,
        py: Python<'_>,
        groups: Vec<String>,
        msg: Py<PyAny>,
        exclude_self: bool,
    ) -> PyResult<()> {
        let bound = msg.bind(py);
        let message = Arc::new(Message::try_from(bound)?);

        let exclude = if exclude_self { self.id } else { None };

        self.channels
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("ChannelStore is not set"))?
            .broadcast(&groups, message, exclude);

        Ok(())
    }

    #[pyo3(signature = (groups, msg, exclude_self=false))]
    fn abroadcast<'py>(
        &self,
        py: Python<'py>,
        groups: Vec<String>,
        msg: Py<PyAny>,
        exclude_self: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let bound = msg.bind(py);
        let message = Arc::new(Message::try_from(bound)?);

        let exclude = if exclude_self { self.id } else { None };

        self.channels
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("ChannelStore is not set"))?
            .broadcast(&groups, message, exclude);

        noop_coroutine(py)
    }

    fn join(&self, group: String) -> PyResult<bool> {
        let channels = self
            .channels
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("ChannelStore is not set"))?;

        let conn_id = self
            .id
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        Ok(channels.join(conn_id, &group))
    }

    fn leave(&self, group: String) -> PyResult<bool> {
        let channels = self
            .channels
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("ChannelStore is not set"))?;

        let conn_id = self
            .id
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        Ok(channels.leave(conn_id, &group))
    }

    fn groups(&self) -> PyResult<Vec<String>> {
        let channels = self
            .channels
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("ChannelStore is not set"))?;

        let conn_id = self
            .id
            .ok_or_else(|| PyRuntimeError::new_err("connection not established"))?;

        Ok(channels.get_groups(conn_id))
    }

    fn group_size(&self, group: String) -> PyResult<usize> {
        let channels = self
            .channels
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("ChannelStore is not set"))?;

        Ok(channels.group_size(&group))
    }
}

#[inline]
fn noop_coroutine(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    Ok(ASYNC_NOOP
        .get()
        .ok_or_else(|| PyRuntimeError::new_err("utils.noop not imported"))?
        .call0(py)?
        .into_bound(py))
}
