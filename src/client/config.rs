use pyo3::prelude::*;
use std::collections::HashMap;

#[pyclass]
#[derive(Default, Clone)]
pub(crate) struct ClientConfig {
    #[pyo3(get)]
    pub(crate) extra_headers: Option<HashMap<String, String>>,
}

#[pymethods]
impl ClientConfig {
    #[new]
    #[pyo3(signature=(extra_headers=None))]
    fn __new__(extra_headers: Option<HashMap<String, String>>) -> Self {
        ClientConfig { extra_headers }
    }
}
