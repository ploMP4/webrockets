use pyo3::exceptions::PyValueError;
use pyo3::{prelude::*, types::PyFunction};

use crate::callback::Callback;
use crate::receive_handler::ReceiveHandler;

#[pyclass]
pub(crate) struct SocketView {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    group: String,
    #[pyo3(get)]
    pub(crate) discriminator: String,
    pub(crate) authentication_classes: Vec<Py<PyAny>>,
    pub(crate) connect_before_callback: Option<Callback>,
    pub(crate) connect_after_callback: Option<Callback>,
    pub(crate) receive_handlers: Vec<ReceiveHandler>,
    pub(crate) generic_receive: Option<Callback>,
    pub(crate) disconnect_callback: Option<Callback>,
}

#[pyclass]
pub(crate) struct ConnectDecorator {
    view: Py<SocketView>,
    before: bool,
}

#[pyclass]
pub(crate) struct ReceiveDecorator {
    view: Py<SocketView>,
    match_value: String,
    schema: Option<Py<PyAny>>,
}

impl SocketView {
    pub(crate) fn new(
        path: String,
        group: String,
        authentication_classes: Vec<Py<PyAny>>,
        discriminator: String,
    ) -> Self {
        Self {
            path,
            group,
            discriminator,
            authentication_classes,
            connect_before_callback: None,
            connect_after_callback: None,
            receive_handlers: Vec::new(),
            generic_receive: None,
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

    #[pyo3(signature = (r#match, /, schema=None))]
    fn receive_match(
        slf: Py<Self>,
        r#match: String,
        schema: Option<Py<PyAny>>,
    ) -> PyResult<ReceiveDecorator> {
        if r#match.is_empty() {
            return Err(PyValueError::new_err("match cannot be emtpy"));
        }

        Ok(ReceiveDecorator {
            view: slf,
            match_value: r#match,
            schema,
        })
    }

    fn receive(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.generic_receive = Some(Callback::new(
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

#[pymethods]
impl ReceiveDecorator {
    fn __call__(&self, py: Python<'_>, func: Py<PyFunction>) -> PyResult<Py<PyFunction>> {
        if let Some(schema) = &self.schema {
            if !schema.bind(py).hasattr("model_validate_json")? {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "schema must be a Pydantic model. Install pywsrs[schema] for Pydantic support.",
                ));
            }
        }

        let mut view = self.view.borrow_mut(py);
        let is_async = view._is_async(py, func.clone_ref(py));

        let handler = ReceiveHandler::new(
            Callback::new(func.clone_ref(py), is_async),
            self.match_value.clone(),
            self.schema.as_ref().map(|s| s.clone_ref(py)),
        );

        view.receive_handlers.push(handler);

        Ok(func)
    }
}
