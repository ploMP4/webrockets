use pyo3::prelude::*;

use crate::callback::Callback;

pub(crate) struct ReceiveHandler {
    pub callback: Callback,
    pub match_value: String,
    pub schema: Option<Py<PyAny>>,
}

impl ReceiveHandler {
    pub(crate) fn new(callback: Callback, match_value: String, schema: Option<Py<PyAny>>) -> Self {
        Self {
            callback,
            match_value,
            schema,
        }
    }

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            callback: self.callback.clone_ref(py),
            match_value: self.match_value.clone(),
            schema: self.schema.as_ref().map(|s| s.clone_ref(py)),
        }
    }

    #[inline(always)]
    pub(crate) fn matches(&self, discriminator_value: &str) -> bool {
        self.match_value == discriminator_value
    }
}
