use pyo3::{prelude::*, types::PyFunction};

use crate::callback::Callback;

#[pyclass]
pub(crate) struct SocketView {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    group: String,
    pub(crate) authentication_classes: Vec<Py<PyAny>>,
    pub(crate) connect_callback: Option<Callback>,
    pub(crate) receive_callback: Option<Callback>,
    pub(crate) disconnect_callback: Option<Callback>,
}

impl SocketView {
    pub(crate) fn new(path: String, group: String, authentication_classes: Vec<Py<PyAny>>) -> Self {
        Self {
            path: path,
            group: group,
            authentication_classes: authentication_classes,
            connect_callback: None,
            receive_callback: None,
            disconnect_callback: None,
        }
    }
}

#[pymethods]
impl SocketView {
    fn _is_async(&self, py: Python<'_>, func: Py<PyFunction>) -> bool {
        py.import("inspect")
            .expect("unable to import inspect module")
            .call_method1("iscoroutinefunction", (func.clone_ref(py),))
            .expect("unable to call inspect.iscoroutinefunction")
            .extract()
            .expect("unable to extract type")
    }

    fn connect(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.connect_callback = Some(Callback::new(
            func.clone_ref(py),
            self._is_async(py, func.clone_ref(py)),
        ));
        func
    }

    fn receive(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.receive_callback = Some(Callback::new(
            func.clone_ref(py),
            self._is_async(py, func.clone_ref(py)),
        ));
        func
    }

    fn disconnect(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.disconnect_callback = Some(Callback::new(
            func.clone_ref(py),
            self._is_async(py, func.clone_ref(py)),
        ));
        func
    }
}
