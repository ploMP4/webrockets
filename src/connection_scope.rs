use std::collections::HashMap;

use pyo3::prelude::*;

#[pyclass]
pub struct ConnectionScope {
    #[pyo3(get)]
    pub path: String,
    #[pyo3(get)]
    pub query_string: String,
    #[pyo3(get)]
    pub headers: HashMap<String, String>,
    #[pyo3(get)]
    pub cookies: HashMap<String, String>,
    #[pyo3(get)]
    pub user: Option<Py<PyAny>>,
}

#[pymethods]
impl ConnectionScope {
    #[new]
    fn __new__(
        path: String,
        query_string: String,
        headers: HashMap<String, String>,
        cookies: HashMap<String, String>,
    ) -> Self {
        ConnectionScope {
            path,
            query_string,
            headers,
            cookies,
            user: None,
        }
    }

    fn get_cookie(&self, name: String) -> Option<&String> {
        self.cookies.get(&name)
    }

    fn get_header(&self, name: String) -> Option<&String> {
        self.headers.get(&name)
    }
}
