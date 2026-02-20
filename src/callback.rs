use pyo3::call::PyCallArgs;
use pyo3::prelude::*;
use pyo3::types::PyFunction;
use tokio::task::JoinHandle;

use crate::connection::Connection;
use crate::TASK_LOCALS;

pub(crate) struct Callback {
    func: Py<PyFunction>,
    is_async: bool,
}

impl Callback {
    pub(crate) fn new(func: Py<PyFunction>, is_async: bool) -> Self {
        Self { func, is_async }
    }

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            func: self.func.clone_ref(py),
            is_async: self.is_async,
        }
    }

    #[inline(always)]
    pub(crate) fn invoke<'py, A>(
        &self,
        py: Python<'py>,
        args: A,
    ) -> PyResult<Option<JoinHandle<PyResult<Py<PyAny>>>>>
    where
        A: PyCallArgs<'py>,
    {
        if self.is_async {
            let coro = self.func.bind(py).call1(args)?;
            let locals = TASK_LOCALS.get().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("no running event loop")
            })?;
            Ok(Some(tokio::spawn(
                pyo3_async_runtimes::into_future_with_locals(locals, coro)?,
            )))
        } else {
            self.func.call1(py, args)?;
            Ok(None)
        }
    }

    #[inline(always)]
    pub(crate) fn invoke_with_schema(
        &self,
        py: Python<'_>,
        args: (&Py<Connection>, &str),
        schema: &Py<PyAny>,
    ) -> PyResult<Option<JoinHandle<PyResult<Py<PyAny>>>>> {
        let (conn, json_str) = args;

        let validated = schema
            .bind(py)
            .call_method1("model_validate_json", (json_str,))?;

        if self.is_async {
            let coro = self.func.bind(py).call1((conn, validated))?;
            let locals = TASK_LOCALS.get().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("no running event loop")
            })?;
            Ok(Some(tokio::spawn(
                pyo3_async_runtimes::into_future_with_locals(locals, coro)?,
            )))
        } else {
            self.func.call1(py, (conn, validated))?;
            Ok(None)
        }
    }
}
