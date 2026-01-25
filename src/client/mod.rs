use std::{future::Future, sync::LazyLock};

use pyo3::prelude::*;
use tokio::runtime::Runtime;

mod r#async;
mod sync;

pub(super) static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("unable to start client runtime"));

pub(super) struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

#[pymodule]
pub(crate) mod client {
    #[pymodule_export]
    use super::r#async::async_client;
    #[pymodule_export]
    use super::sync::sync;
}
