use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyBytes, PyString},
};
use pyo3_async_runtimes::TaskLocals;
use std::sync::{Arc, OnceLock};

mod callback;
mod channel_store;
mod connection;
mod server;
mod socket_view;

static TASK_LOCALS: OnceLock<TaskLocals> = OnceLock::new();
static RUN_CORO_THREADSAFE: OnceLock<Py<PyAny>> = OnceLock::new();
static ASYNCIO_SLEEP: OnceLock<Py<PyAny>> = OnceLock::new();

fn start_python_event_loop(py: Python<'_>) -> PyResult<()> {
    let runtime_builder = tokio::runtime::Builder::new_multi_thread();
    pyo3_async_runtimes::tokio::init(runtime_builder);

    let asyncio = py.import("asyncio")?;
    let run_coro = asyncio.getattr("run_coroutine_threadsafe")?.unbind();
    let _ = RUN_CORO_THREADSAFE.set(run_coro);
    let sleep_fn = asyncio.getattr("sleep")?.unbind();
    let _ = ASYNCIO_SLEEP.set(sleep_fn);

    let loop_obj: Py<PyAny> = {
        let ev = match py.import("uvloop") {
            Ok(uvloop) => uvloop.call_method0("new_event_loop")?,
            Err(_) => asyncio.call_method0("new_event_loop")?,
        };
        let locals = pyo3_async_runtimes::TaskLocals::new(ev.clone()).copy_context(py)?;
        let _ = TASK_LOCALS.set(locals);
        ev.unbind().into()
    };
    std::thread::spawn(move || {
        Python::attach(|py| {
            let asyncio = py.import("asyncio").expect("import asyncio");
            let ev = loop_obj.bind(py);
            let _ = asyncio.call_method1("set_event_loop", (ev.as_any(),));
            let _ = ev.call_method0("run_forever");
        });
    });

    Ok(())
}

#[derive(Debug)]
enum Message {
    Text(Arc<str>),
    Binary(Arc<[u8]>),
    Close(u16, Arc<[u8]>),
}

impl TryFrom<&Bound<'_, PyAny>> for Message {
    type Error = PyErr;

    fn try_from(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        if value.is_instance_of::<PyString>() {
            let s: &str = value.extract()?;
            Ok(Message::Text(s.into()))
        } else if value.is_instance_of::<PyBytes>() {
            let b: &[u8] = value.extract()?;
            Ok(Message::Binary(b.into()))
        } else {
            Err(PyTypeError::new_err("send() argument must be str or bytes"))
        }
    }
}

#[pymodule]
mod django_wsrs {
    use pyo3::prelude::*;

    #[pymodule_export]
    use super::connection::BaseConnection;
    #[pymodule_export]
    use super::connection::Connection;
    #[pymodule_export]
    use super::connection::IncomingConnection;
    #[pymodule_export]
    use super::server::WebsocketServer;
    #[pymodule_export]
    use super::socket_view::ConnectDecorator;
    #[pymodule_export]
    use super::socket_view::SocketView;

    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        pyo3_log::init();
        m.add("Websocket", WebsocketServer::new())?;
        Ok(())
    }
}
