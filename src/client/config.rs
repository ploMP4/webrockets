use pyo3::prelude::*;
use std::collections::HashMap;

#[pyclass]
#[derive(Default, Clone)]
pub(crate) struct ClientConfig {
    #[pyo3(get)]
    pub(crate) extra_headers: Option<HashMap<String, String>>,
    #[pyo3(get)]
    pub(crate) max_message_size: Option<usize>,
    #[pyo3(get)]
    pub(crate) verify_ssl: bool,
    #[pyo3(get)]
    pub(crate) subprotocols: Option<Vec<String>>,
}

#[pymethods]
impl ClientConfig {
    #[new]
    #[pyo3(signature=(extra_headers=None, max_message_size=None, verify_ssl=false, subprotocols=None))]
    fn __new__(
        extra_headers: Option<HashMap<String, String>>,
        max_message_size: Option<usize>,
        verify_ssl: bool,
        subprotocols: Option<Vec<String>>,
    ) -> Self {
        ClientConfig {
            extra_headers,
            max_message_size,
            verify_ssl,
            subprotocols,
        }
    }
}
