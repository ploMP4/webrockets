use pyo3::prelude::*;

use crate::callback::Callback;

pub(crate) struct ReceiveHandler {
    pub(crate) callback: Callback,
    pub(crate) schema: Option<Py<PyAny>>,
    pub(crate) remove_key: bool,
}

impl ReceiveHandler {
    pub(crate) fn new(
        callback: Callback,
        schema: Option<Py<PyAny>>,
        remove_key: bool,
    ) -> Self {
        Self {
            callback,
            schema,
            remove_key,
        }
    }
}
