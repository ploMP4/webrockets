use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyBytes, PyString},
};
use pyo3_async_runtimes::TaskLocals;
use std::sync::{Arc, OnceLock};

mod broker;
mod callback;
mod channel_store;
mod connection;
mod receive_handler;
mod route;
mod server;

static TASK_LOCALS: OnceLock<TaskLocals> = OnceLock::new();
static ASYNC_NOOP: OnceLock<Py<PyAny>> = OnceLock::new();

fn start_python_event_loop(py: Python<'_>) -> PyResult<()> {
    let mut runtime_builder = tokio::runtime::Builder::new_multi_thread();
    runtime_builder.enable_all();
    pyo3_async_runtimes::tokio::init(runtime_builder);

    let asyncio = py.import("asyncio")?;

    let utils = py.import("pywsrs.utils")?;
    let noop = utils.getattr("noop")?.unbind();
    let _ = ASYNC_NOOP.set(noop);

    let loop_obj: Py<PyAny> = {
        let ev = match py.import("uvloop") {
            Ok(uvloop) => uvloop.call_method0("new_event_loop")?,
            Err(_) => asyncio.call_method0("new_event_loop")?,
        };
        let locals = pyo3_async_runtimes::TaskLocals::new(ev.clone()).copy_context(py)?;
        let _ = TASK_LOCALS.set(locals);
        ev.unbind()
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
mod pywsrs {
    use pyo3::prelude::*;

    #[pymodule_export]
    use super::broker::abroadcast;
    #[pymodule_export]
    use super::broker::broadcast;
    #[pymodule_export]
    use super::broker::reset_broadcast;
    #[pymodule_export]
    use super::broker::setup_broadcast;
    #[pymodule_export]
    use super::connection::BaseConnection;
    #[pymodule_export]
    use super::connection::Connection;
    #[pymodule_export]
    use super::connection::IncomingConnection;
    #[pymodule_export]
    use super::route::ConnectDecorator;
    #[pymodule_export]
    use super::route::Match;
    #[pymodule_export]
    use super::route::ReceiveDecorator;
    #[pymodule_export]
    use super::route::WebsocketRoute;
    #[pymodule_export]
    use super::server::WebsocketServer;

    #[pymodule_init]
    fn init(_: &Bound<'_, PyModule>) -> PyResult<()> {
        pyo3_log::init();
        Ok(())
    }
}
