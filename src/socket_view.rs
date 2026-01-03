use pyo3::{prelude::*, types::PyFunction};

use crate::callback::Callback;

#[pyclass]
pub(crate) struct SocketView {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    group: String,
    pub(crate) authentication_classes: Vec<Py<PyAny>>,
    pub(crate) connect_before_callback: Option<Callback>,
    pub(crate) connect_after_callback: Option<Callback>,
    pub(crate) receive_callback: Option<Callback>,
    pub(crate) disconnect_callback: Option<Callback>,
}

#[pyclass]
pub(crate) struct ConnectDecorator {
    view: Py<SocketView>,
    before: bool,
}

impl SocketView {
    pub(crate) fn new(path: String, group: String, authentication_classes: Vec<Py<PyAny>>) -> Self {
        Self {
            path,
            group,
            authentication_classes,
            connect_before_callback: None,
            connect_after_callback: None,
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

    fn connect(slf: Py<Self>, when: &str) -> PyResult<ConnectDecorator> {
        let before = match when {
            "before" => true,
            "after" => false,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "connect() argument must be 'before' or 'after'",
                ))
            }
        };
        Ok(ConnectDecorator { view: slf, before })
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

#[pymethods]
impl ConnectDecorator {
    fn __call__(&self, py: Python<'_>, func: Py<PyFunction>) -> PyResult<Py<PyFunction>> {
        let mut view = self.view.borrow_mut(py);
        let is_async = view._is_async(py, func.clone_ref(py));
        let callback = Callback::new(func.clone_ref(py), is_async);

        if self.before {
            view.connect_before_callback = Some(callback);
        } else {
            view.connect_after_callback = Some(callback);
        }

        Ok(func)
    }
}
