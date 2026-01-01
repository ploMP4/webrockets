use pyo3::call::PyCallArgs;
use pyo3::prelude::*;
use pyo3::types::PyFunction;

use crate::{RUN_CORO_THREADSAFE, TASK_LOCALS};

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
    pub(crate) fn invoke<'py, A>(&self, py: Python<'py>, args: A) -> PyResult<()>
    where
        A: PyCallArgs<'py>,
    {
        if self.is_async {
            let coro = self.func.call1(py, args)?;
            let locals = TASK_LOCALS.get().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Asyncio loop not initialized")
            })?;
            let run_coro = RUN_CORO_THREADSAFE.get().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    "run_coroutine_threadsafe not initialized",
                )
            })?;
            run_coro.call1(py, (coro, locals.event_loop(py)))?;
        } else {
            self.func.call1(py, args)?;
        }

        Ok(())
    }
}
