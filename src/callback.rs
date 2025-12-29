use pyo3::call::PyCallArgs;
use pyo3::prelude::*;
use pyo3::types::PyFunction;

use crate::{RUN_CORO_THREADSAFE, TASK_LOCALS};

pub struct Callback {
    pub func: Py<PyFunction>,
    pub is_async: bool,
}

impl Callback {
    pub fn new(func: Py<PyFunction>, is_async: bool) -> Self {
        Self {
            func: func,
            is_async: is_async,
        }
    }

    #[inline(always)]
    pub fn invoke<'py, A>(&self, py: Python<'py>, args: A) -> PyResult<()>
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
