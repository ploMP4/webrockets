use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::{routing::get, Router};
use dashmap::DashMap;
use fastwebsockets::{upgrade, Frame, OpCode, WebSocketError};
use pyo3::call::PyCallArgs;
use pyo3::prelude::*;
use pyo3::types::{PyFunction, PyInt};
use pyo3_async_runtimes::TaskLocals;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, watch, RwLock};

const CHANNEL_BUFFER_SIZE: usize = 10000;
static TASK_LOCALS: OnceLock<TaskLocals> = OnceLock::new();
static RUN_CORO_THREADSAFE: OnceLock<Py<PyAny>> = OnceLock::new();
static ASYNCIO_SLEEP: OnceLock<Py<PyAny>> = OnceLock::new();

fn start_python_event_loop(py: Python<'_>) -> PyResult<()> {
    let runtime_builder = tokio::runtime::Builder::new_multi_thread();
    pyo3_async_runtimes::tokio::init(runtime_builder);

    let asyncio = py.import("asyncio")?;
    let run_coro = asyncio.getattr("run_coroutine_threadsafe")?.unbind();
    let _ = RUN_CORO_THREADSAFE.set(run_coro);
    let sleep_fn = asyncio.getattr("sleep")?.unbind();
    let _ = ASYNCIO_SLEEP.set(sleep_fn);

    let loop_obj: Py<PyAny> = {
        let ev = match py.import("uvloop") {
            Ok(uvloop) => uvloop.call_method0("new_event_loop")?,
            Err(_) => asyncio.call_method0("new_event_loop")?,
        };
        let locals = pyo3_async_runtimes::TaskLocals::new(ev.clone()).copy_context(py)?;
        let _ = TASK_LOCALS.set(locals);
        ev.unbind().into()
    };
    std::thread::spawn(move || {
        Python::attach(|py| {
            let asyncio = py.import("asyncio").expect("import asyncio");
            let ev = loop_obj.bind(py);
            let _ = asyncio.call_method1("set_event_loop", (ev.as_any(),));
            let _ = ev.call_method0("run_forever");
        });
    });

    Ok(())
}

struct AppState {
    channels: ChannelStore,
    registry: RwLock<HashMap<String, Arc<Py<SocketView>>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            channels: ChannelStore::new(),
            registry: RwLock::new(HashMap::new()),
        }
    }
}

#[pyclass]
struct ConnectionScope {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    query_string: String,
    #[pyo3(get)]
    headers: HashMap<String, String>,
    #[pyo3(get)]
    cookies: HashMap<String, String>,
    #[pyo3(get)]
    user: Option<Py<PyAny>>,
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

#[derive(Debug)]
#[allow(dead_code)]
enum Message {
    Text(Arc<str>),
    Binary(Arc<[u8]>),
    Close(),
}

#[pyclass]
struct WebsocketServer {
    rt: Runtime,
    state: Arc<AppState>,
    context: Vec<(String, String)>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl WebsocketServer {
    fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            rt: Runtime::new().expect("Unable to create tokio runtime"),
            state: Arc::new(AppState::new()),
            context: Vec::new(),
            shutdown_tx,
            shutdown_rx,
        }
    }

    fn runserver(&self, host: &str, port: &str) {
        let mut shutdown_rx = self.shutdown_rx.clone();

        self.rt.block_on(async {
            let mut app = Router::new();
            for (group, path) in self.context.clone() {
                app = app.route(
                    &format!("/{path}"),
                    get(
                        |uri: Uri,
                         headers: HeaderMap,
                         ws: upgrade::IncomingUpgrade,
                         State(state): State<Arc<AppState>>| async move {
                            let scope = WebsocketServer::extract_scope(&uri, &headers);

                            let handler = state.registry.read().await.get(&path).cloned();
                            if let Some(ref handler) = handler {
                                let handler_clone = Arc::clone(handler);
                                let scope_clone = Python::attach(|py| scope.clone_ref(py));
                                let combined_result = tokio::task::spawn_blocking(move || {
                                    Python::attach(|py| -> Result<(), (StatusCode, &'static str)> {
                                        let view = handler_clone.borrow(py);
                                        let auth_classes = &view.authentication_classes;

                                        if !WebsocketServer::authenticated(
                                            py,
                                            auth_classes,
                                            &scope_clone,
                                        ) {
                                            return Err((
                                                StatusCode::UNAUTHORIZED,
                                                "Authentication failed",
                                            ));
                                        }

                                        if let Some(cb) = &view.connect_callback {
                                            if let Err(e) = cb.invoke(py, (&scope_clone,)) {
                                                log::error!(
                                                    "Error in websocket connect callback: {}",
                                                    e
                                                );
                                                return Err((
                                                    StatusCode::INTERNAL_SERVER_ERROR,
                                                    "Connection failed",
                                                ));
                                            }
                                        }

                                        Ok(())
                                    })
                                })
                                .await;

                                match combined_result {
                                    Ok(Err((status, msg))) => {
                                        return (status, msg).into_response();
                                    }
                                    Err(_) => {
                                        return (
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            "Internal error",
                                        )
                                            .into_response();
                                    }
                                    Ok(Ok(())) => {}
                                }

                                let (response, fut) = ws.upgrade().unwrap();
                                let handler_clone = Arc::clone(handler);

                                tokio::spawn(async move {
                                    if let Err(e) = WebsocketServer::handle_client(
                                        fut,
                                        group,
                                        state,
                                        scope,
                                        handler_clone,
                                    )
                                    .await
                                    {
                                        log::error!("Error in websocket connection: {}", e);
                                    }
                                });

                                return response.into_response();
                            }

                            (StatusCode::NOT_FOUND, "Handler not found").into_response()
                        },
                    ),
                );
            }

            let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
                .await
                .unwrap();

            log::info!("listening on {}", listener.local_addr().unwrap());

            axum::serve(listener, app.with_state(Arc::clone(&self.state)))
                .with_graceful_shutdown(async move {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {
                            log::info!("Ctrl+C received, shutting down...");
                        }
                        _ = shutdown_rx.changed() => {
                            log::info!("Stop signal received, shutting down...");
                        }
                    }
                })
                .await
                .unwrap();
        });
    }

    fn extract_scope(uri: &Uri, header_map: &HeaderMap) -> Py<ConnectionScope> {
        let path = uri.path().to_owned();
        let query_string = uri.query().unwrap_or_default().to_owned();

        let mut headers = HashMap::with_capacity(header_map.len());
        for (name, value) in header_map.iter() {
            if let Ok(v) = value.to_str() {
                headers.insert(name.as_str().to_lowercase(), v.to_owned());
            }
        }

        let mut cookies = HashMap::new();
        if let Some(cookie_header) = header_map.get("cookie") {
            if let Ok(cookie_str) = cookie_header.to_str() {
                cookies.reserve(4);
                for cookie in cookie_str.split(';') {
                    let cookie = cookie.trim();
                    if let Some((name, value)) = cookie.split_once('=') {
                        cookies.insert(name.trim().to_owned(), value.trim().to_owned());
                    }
                }
            }
        }

        Python::attach(|py| -> Py<ConnectionScope> {
            Py::new(
                py,
                ConnectionScope {
                    path,
                    headers,
                    cookies,
                    query_string,
                    user: None,
                },
            )
            .expect("Unable to create connection scope")
        })
    }

    fn authenticated(
        py: Python<'_>,
        auth_classes: &Vec<Py<PyAny>>,
        scope: &Py<ConnectionScope>,
    ) -> bool {
        if auth_classes.is_empty() {
            return true;
        }

        for authenticator in auth_classes {
            if let Ok(auth) = authenticator.getattr(py, "authenticate") {
                if let Some(res) = auth
                    .call1(py, (&scope,))
                    .ok()
                    .filter(|res| !res.is_none(py))
                {
                    scope.borrow_mut(py).user = Some(res);
                    return true;
                }
            }
        }
        false
    }

    async fn send_listener<S>(
        mut rx: mpsc::Receiver<Arc<Message>>,
        mut writer: fastwebsockets::WebSocketWrite<S>,
    ) where
        S: Unpin + tokio::io::AsyncWrite,
    {
        while let Some(msg) = rx.recv().await {
            match msg.as_ref() {
                Message::Text(text) => {
                    let frame = Frame::text(fastwebsockets::Payload::Borrowed(text.as_bytes()));
                    if writer.write_frame(frame).await.is_err() {
                        break;
                    }
                }
                Message::Binary(data) => {
                    let frame = Frame::binary(fastwebsockets::Payload::Borrowed(&data));
                    if writer.write_frame(frame).await.is_err() {
                        break;
                    }
                }
                Message::Close() => {
                    // let frame = Frame::close(code, reason);
                    // if tx.write_frame(frame).await.is_err() {
                    //     break;
                    // }
                    break;
                }
            }
        }
    }

    async fn receive_listener<S>(
        mut reader: fastwebsockets::FragmentCollectorRead<S>,
        scope: Py<ConnectionScope>,
        channel_id: Py<PyInt>,
        receive_callback: Option<Callback>,
        disconnect_callback: Option<Callback>,
    ) where
        S: Unpin + tokio::io::AsyncRead,
    {
        loop {
            let frame = reader
                .read_frame::<_, WebSocketError>(&mut |_| async { Ok(()) })
                .await;

            match frame {
                Ok(frame) => match frame.opcode {
                    OpCode::Close => {
                        if let Some(cb) = &disconnect_callback {
                            let payload_bytes = &frame.payload[..];
                            let close_code = frame.opcode as u16;
                            let payload_str = std::str::from_utf8(payload_bytes);

                            if let Err(e) = Python::attach(|py| match payload_str {
                                Ok(s) => cb.invoke(py, (&scope, close_code, s)),
                                Err(_) => {
                                    let s = String::from_utf8_lossy(payload_bytes);
                                    cb.invoke(py, (&scope, close_code, s))
                                }
                            }) {
                                log::error!("Error in disconnect callback: {}", e);
                            }
                        }
                        break;
                    }
                    OpCode::Text | OpCode::Binary => {
                        if let Some(cb) = &receive_callback {
                            let payload_bytes = &frame.payload[..];
                            let payload_str = std::str::from_utf8(payload_bytes);

                            if let Err(e) = Python::attach(|py| match payload_str {
                                Ok(s) => cb.invoke(py, (&scope, &channel_id, s)),
                                Err(_) => {
                                    let s = String::from_utf8_lossy(payload_bytes);
                                    cb.invoke(py, (&scope, &channel_id, s))
                                }
                            }) {
                                log::error!("Error in receive callback: {}", e);
                            }
                        }
                    }
                    _ => {}
                },
                Err(_) => break,
            }
        }
    }

    async fn handle_client<'a>(
        fut: upgrade::UpgradeFut,
        group: String,
        state: Arc<AppState>,
        scope: Py<ConnectionScope>,
        handler: Arc<Py<SocketView>>,
    ) -> Result<(), WebSocketError> {
        let (reader, writer) = fut.await?.split(tokio::io::split);
        let reader = fastwebsockets::FragmentCollectorRead::new(reader);

        let (tx, rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let channel_id = state.channels.register(&group, tx);

        let (receive_cb, disconnect_cb, channel_id_py) = Python::attach(|py| {
            let view = handler.borrow(py);
            (
                view.receive_callback.as_ref().map(|cb| Callback {
                    func: cb.func.clone_ref(py),
                    is_async: cb.is_async,
                }),
                view.disconnect_callback.as_ref().map(|cb| Callback {
                    func: cb.func.clone_ref(py),
                    is_async: cb.is_async,
                }),
                channel_id.into_pyobject(py).unwrap().unbind(),
            )
        });

        let mut send_task = tokio::spawn(WebsocketServer::send_listener(rx, writer));

        let mut receive_task = tokio::spawn(WebsocketServer::receive_listener(
            reader,
            scope,
            channel_id_py,
            receive_cb,
            disconnect_cb,
        ));

        tokio::select! {
            _ = &mut send_task => receive_task.abort(),
            _ = &mut receive_task => send_task.abort(),
        }

        state.channels.remove(channel_id);
        Ok(())
    }
}

#[pymethods]
impl WebsocketServer {
    fn start(&self, py: Python<'_>) -> PyResult<()> {
        let settings = py.import("django.conf")?.getattr("settings")?;

        let host: String = settings
            .getattr("WEBSOCKET_HOST")
            .and_then(|v| v.extract())
            .unwrap_or("0.0.0.0".to_string());

        let port: String = settings
            .getattr("WEBSOCKET_PORT")
            .and_then(|v| {
                v.extract::<String>()
                    .or_else(|_| v.extract::<u16>().map(|n| n.to_string()))
            })
            .unwrap_or("46290".to_string());

        start_python_event_loop(py)?;
        py.detach(|| self.runserver(&host, &port));
        Ok(())
    }

    fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    #[pyo3(signature = (path, group, authentication_classes = None))]
    fn __call__(
        &mut self,
        py: Python<'_>,
        path: String,
        group: String,
        authentication_classes: Option<Vec<Py<PyAny>>>,
    ) -> PyResult<Py<SocketView>> {
        self.context.push((group.clone(), path.clone()));
        let instance = Py::new(
            py,
            SocketView::new(
                path.clone(),
                group,
                authentication_classes.unwrap_or(Vec::new()),
            ),
        )?;
        let instance_ref = instance.clone_ref(py);
        py.detach(|| {
            self.rt.block_on(async {
                self.state
                    .registry
                    .write()
                    .await
                    .insert(path, Arc::new(instance_ref))
            })
        });
        Ok(instance)
    }

    fn asend<'py>(&self, py: Python<'py>, cid: u64, msg: String) -> PyResult<Bound<'py, PyAny>> {
        let message = Arc::new(Message::Text(msg.into()));

        if let Some(()) = self.state.channels.try_send(cid, &message) {
            let sleep = ASYNCIO_SLEEP.get().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("asyncio.sleep not initialized")
            })?;
            return Ok(sleep.call1(py, (0,))?.into_bound(py));
        }

        let state = Arc::clone(&self.state);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let _ = state.channels.send(cid, message).await;
            Ok(())
        })
    }

    fn send(&self, py: Python<'_>, cid: u64, msg: String) -> PyResult<()> {
        let message = Arc::new(Message::Text(msg.into()));

        if let Some(()) = self.state.channels.try_send(cid, &message) {
            return Ok(());
        }

        py.detach(|| {
            let state = Arc::clone(&self.state);
            tokio::spawn(async move {
                let _ = state.channels.send(cid, message).await;
            });
        });
        Ok(())
    }

    fn broadcast_text(&self, py: Python<'_>, groups: Vec<String>, msg: String) {
        py.detach(|| {
            self.state
                .channels
                .broadcast(&groups, Arc::new(Message::Text(msg.into())));
        });
    }
}

struct ChannelStore {
    next_id: AtomicU64,
    data: DashMap<u64, Arc<mpsc::Sender<Arc<Message>>>>,
    grouped: DashMap<String, Vec<Arc<mpsc::Sender<Arc<Message>>>>>,
}

impl ChannelStore {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            data: DashMap::new(),
            grouped: DashMap::new(),
        }
    }

    fn register(&self, group: &str, value: mpsc::Sender<Arc<Message>>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let ch_arc: Arc<mpsc::Sender<Arc<Message>>> = value.into();

        self.data.insert(id, Arc::clone(&ch_arc));
        self.grouped
            .entry(group.to_string())
            .or_insert_with(Vec::new)
            .push(ch_arc);

        id
    }

    fn remove(&self, id: u64) {
        self.data.remove(&id);
    }

    fn try_send(&self, id: u64, msg: &Arc<Message>) -> Option<()> {
        if let Some(tx) = self.data.get(&id) {
            match tx.try_send(Arc::clone(msg)) {
                Ok(()) => Some(()),
                Err(mpsc::error::TrySendError::Full(_)) => None,
                Err(mpsc::error::TrySendError::Closed(_)) => Some(()),
            }
        } else {
            Some(())
        }
    }

    async fn send(
        &self,
        id: u64,
        msg: Arc<Message>,
    ) -> Result<(), mpsc::error::SendError<Arc<Message>>> {
        if let Some(tx) = self.data.get(&id) {
            tx.send(msg).await?;
        };
        Ok(())
    }

    fn broadcast(&self, groups: &[String], msg: Arc<Message>) {
        let capacity: usize = groups
            .iter()
            .filter_map(|g| self.grouped.get(g).map(|e| e.len()))
            .sum();

        if capacity == 0 {
            return;
        }

        let mut blocked_senders = Vec::new();
        for group in groups {
            if let Some(entry) = self.grouped.get(group) {
                for tx in entry.iter() {
                    match tx.try_send(Arc::clone(&msg)) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            blocked_senders.push(Arc::clone(tx));
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {}
                    }
                }
            }
        }

        if !blocked_senders.is_empty() {
            tokio::spawn(async move {
                for tx in blocked_senders {
                    let _ = tx.send(Arc::clone(&msg)).await;
                }
            });
        }
    }
}

struct Callback {
    func: Py<PyFunction>,
    is_async: bool,
}

impl Callback {
    #[inline(always)]
    fn invoke<'py, A>(&self, py: Python<'py>, args: A) -> PyResult<()>
    where
        A: PyCallArgs<'py>,
    {
        if self.is_async {
            let coro = self.func.call1(py, args)?;
            let locals = TASK_LOCALS.get().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Asyncio loop not initialized")
            })?;
            let run_coro = RUN_CORO_THREADSAFE.get().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    "run_coroutine_threadsafe not initialized",
                )
            })?;
            run_coro.call1(py, (coro, locals.event_loop(py)))?;
        } else {
            self.func.call1(py, args)?;
        }

        Ok(())
    }
}

#[pyclass]
struct SocketView {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    group: String,
    authentication_classes: Vec<Py<PyAny>>,
    connect_callback: Option<Callback>,
    receive_callback: Option<Callback>,
    disconnect_callback: Option<Callback>,
}

impl SocketView {
    fn new(path: String, group: String, authentication_classes: Vec<Py<PyAny>>) -> Self {
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
        self.connect_callback = Some(Callback {
            func: func.clone_ref(py),
            is_async: self._is_async(py, func.clone_ref(py)),
        });
        func
    }

    fn receive(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.receive_callback = Some(Callback {
            func: func.clone_ref(py),
            is_async: self._is_async(py, func.clone_ref(py)),
        });
        func
    }

    fn disconnect(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.disconnect_callback = Some(Callback {
            func: func.clone_ref(py),
            is_async: self._is_async(py, func.clone_ref(py)),
        });
        func
    }
}

#[pymodule]
mod django_wsrs {
    use pyo3::prelude::*;

    #[pymodule_export]
    use super::ConnectionScope;
    #[pymodule_export]
    use super::SocketView;
    #[pymodule_export]
    use super::WebsocketServer;

    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        pyo3_log::init();
        m.add("Websocket", WebsocketServer::new())?;
        Ok(())
    }
}
