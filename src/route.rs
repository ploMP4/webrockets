use indexmap::IndexMap;
use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::types::PyList;
use pyo3::{prelude::*, types::PyFunction};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, OnceLock, RwLock};

use crate::callback::Callback;
use crate::receive_handler::ReceiveHandler;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum MatchValue {
    /// Matches any value (use "*" in Python)
    Wildcard,
    String(Arc<str>),
    Int(i64),
}

impl MatchValue {
    #[inline]
    pub(crate) fn matches_json(&self, json_val: &serde_json::Value) -> bool {
        match self {
            MatchValue::Wildcard => true,
            MatchValue::String(s) => {
                matches!(json_val, serde_json::Value::String(js) if s.as_ref() == js)
            }
            MatchValue::Int(i) => {
                matches!(json_val, serde_json::Value::Number(n) if n.as_i64() == Some(*i))
            }
        }
    }
}

pub(crate) type FrozenHandlers = Arc<IndexMap<Arc<str>, HashMap<MatchValue, Arc<ReceiveHandler>>>>;

type MutableHandlers = IndexMap<Arc<str>, HashMap<MatchValue, Arc<ReceiveHandler>>>;

#[pyclass]
#[derive(Clone)]
pub struct Match {
    pub(crate) keys: Vec<Arc<str>>,
    pub(crate) values: Vec<MatchValue>,
    #[pyo3(get)]
    pub(crate) remove_key: bool,
}

#[pymethods]
impl Match {
    #[new]
    #[pyo3(signature = (key, value, /, remove_key=false))]
    fn new(key: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>, remove_key: bool) -> PyResult<Self> {
        Ok(Self {
            keys: Self::extract_keys(key)?,
            values: Self::extract_values(value)?,
            remove_key,
        })
    }

    #[getter]
    fn key(&self) -> Vec<String> {
        self.keys.iter().map(|s| s.to_string()).collect()
    }

    #[getter]
    fn value(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for v in &self.values {
            match v {
                MatchValue::Wildcard => list.append("*")?,
                MatchValue::String(s) => list.append(s.as_ref())?,
                MatchValue::Int(i) => list.append(*i)?,
            }
        }
        Ok(list.unbind())
    }
}

impl Match {
    /// Extract either a single string or an iterable of strings into Vec<Arc<str>>
    fn extract_keys(obj: &Bound<'_, PyAny>) -> PyResult<Vec<Arc<str>>> {
        if let Ok(s) = obj.extract::<String>() {
            return Ok(vec![s.into()]);
        }

        let iter = obj
            .try_iter()
            .map_err(|_| PyTypeError::new_err("key must be str or iterable of str"))?;

        let mut result = Vec::new();
        for item in iter {
            let item = item?;
            let s: String = item
                .extract()
                .map_err(|_| PyTypeError::new_err("key iterable must contain only strings"))?;

            result.push(s.into());
        }

        if result.is_empty() {
            return Err(PyValueError::new_err("key iterable must not be empty"));
        }

        Ok(result)
    }

    /// Extract a single value (str, int, or "*" for wildcard) into MatchValue
    fn extract_single_value(obj: &Bound<'_, PyAny>) -> PyResult<MatchValue> {
        if let Ok(s) = obj.extract::<String>() {
            return Ok(if s == "*" {
                MatchValue::Wildcard
            } else {
                MatchValue::String(s.into())
            });
        }
        if let Ok(i) = obj.extract::<i64>() {
            return Ok(MatchValue::Int(i));
        }
        Err(PyTypeError::new_err(
            "value must be str, int, or \"*\" for wildcard",
        ))
    }

    /// Extract either a single value or an iterable of values into Vec<MatchValue>
    fn extract_values(obj: &Bound<'_, PyAny>) -> PyResult<Vec<MatchValue>> {
        if let Ok(v) = Self::extract_single_value(obj) {
            return Ok(vec![v]);
        }

        let iter = obj
            .try_iter()
            .map_err(|_| PyTypeError::new_err("value must be str, int, or iterable of these"))?;

        let mut result = Vec::new();
        for item in iter {
            let item = item?;
            result.push(Self::extract_single_value(&item)?);
        }

        if result.is_empty() {
            return Err(PyValueError::new_err("value iterable must not be empty"));
        }

        Ok(result)
    }
}

#[pyclass]
pub(crate) struct WebsocketRoute {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    default_group: Option<String>,
    #[pyo3(get)]
    pub(crate) authentication_classes: Vec<Py<PyAny>>,
    pub(crate) connect_before_callback: Option<Callback>,
    pub(crate) connect_after_callback: Option<Callback>,
    /// Mutable during registration phase (decorator execution)
    receive_handlers_mut: RwLock<MutableHandlers>,
    /// Frozen snapshot - populated on first access, lock-free thereafter
    pub(crate) receive_handlers: OnceLock<FrozenHandlers>,
    pub(crate) generic_receive: Option<Callback>,
    pub(crate) generic_receive_schema: Option<Py<PyAny>>,
    pub(crate) disconnect_callback: Option<Callback>,
}

#[pyclass]
pub(crate) struct ConnectDecorator {
    view: Py<WebsocketRoute>,
    before: bool,
}

#[pyclass]
pub(crate) struct ReceiveDecorator {
    view: Py<WebsocketRoute>,
    match_spec: Option<Match>,
    schema: Option<Py<PyAny>>,
}

impl WebsocketRoute {
    pub(crate) fn new(
        path: String,
        default_group: Option<String>,
        authentication_classes: Vec<Py<PyAny>>,
    ) -> Self {
        Self {
            path,
            default_group,
            authentication_classes,
            connect_before_callback: None,
            connect_after_callback: None,
            receive_handlers_mut: RwLock::new(IndexMap::new()),
            receive_handlers: OnceLock::new(),
            generic_receive: None,
            generic_receive_schema: None,
            disconnect_callback: None,
        }
    }

    /// Get or create the frozen handlers snapshot.
    /// First call clones from mutable map; subsequent calls return cached Arc.
    #[inline]
    pub(crate) fn frozen_handlers(&self) -> FrozenHandlers {
        self.receive_handlers
            .get_or_init(|| {
                let guard = self.receive_handlers_mut.read().unwrap();
                Arc::new(guard.clone())
            })
            .clone()
    }
}

#[pymethods]
impl WebsocketRoute {
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

    #[pyo3(signature = (func=None, /, r#match=None, schema=None))]
    fn receive(
        slf: Py<Self>,
        py: Python<'_>,
        func: Option<Py<PyFunction>>,
        r#match: Option<Match>,
        schema: Option<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        match (func, r#match) {
            (Some(f), None) => {
                let mut view = slf.borrow_mut(py);
                if view.generic_receive.is_some() {
                    return Err(PyValueError::new_err(
                        "Generic receive handler already registered",
                    ));
                }
                let is_async = view._is_async(py, f.clone_ref(py));
                view.generic_receive = Some(Callback::new(f.clone_ref(py), is_async));
                view.generic_receive_schema = schema;
                Ok(f.into_any())
            }
            (None, match_spec) => {
                let decorator = Py::new(
                    py,
                    ReceiveDecorator {
                        view: slf,
                        match_spec,
                        schema,
                    },
                )?;
                Ok(decorator.into_any())
            }
            (Some(_), Some(_)) => Err(PyValueError::new_err(
                "Cannot use match= with bare decorator syntax",
            )),
        }
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
                return Err(PyTypeError::new_err(
                    "schema must be a Pydantic model. Install pywsrs[schema] for Pydantic support.",
                ));
            }
        }

        let mut view = self.view.borrow_mut(py);
        let is_async = view._is_async(py, func.clone_ref(py));

        match &self.match_spec {
            Some(m) => {
                {
                    let guard = view
                        .receive_handlers_mut
                        .read()
                        .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

                    for key in &m.keys {
                        for value in &m.values {
                            if guard.get(key).is_some_and(|v| v.contains_key(value)) {
                                return Err(PyValueError::new_err(format!(
                                    "Handler for match(key={:?}, value={:?}) already registered",
                                    key, value
                                )));
                            }
                        }
                    }
                }

                // Create handler once, share via Arc for all key/value combinations
                let handler = Arc::new(ReceiveHandler::new(
                    Callback::new(func.clone_ref(py), is_async),
                    self.schema.as_ref().map(|s| s.clone_ref(py)),
                    m.remove_key,
                ));

                // Register for all key/value combinations
                let mut handlers = view
                    .receive_handlers_mut
                    .write()
                    .map_err(|e| PyRuntimeError::new_err(format!("poisoned lock: {e}")))?;

                for key in &m.keys {
                    for value in &m.values {
                        handlers
                            .entry(key.clone())
                            .or_default()
                            .insert(value.clone(), handler.clone());
                    }
                }
            }
            None => {
                if view.generic_receive.is_some() {
                    return Err(PyValueError::new_err(
                        "Generic receive handler already registered",
                    ));
                }
                view.generic_receive = Some(Callback::new(func.clone_ref(py), is_async));
                view.generic_receive_schema = self.schema.as_ref().map(|s| s.clone_ref(py));
            }
        }

        Ok(func)
    }
}
