use dashmap::DashMap;
use fastwebsockets::{upgrade, Frame, OpCode, WebSocketError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::{routing::get, Router};
use pyo3::prelude::*;
use pyo3::types::PyFunction;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, watch, RwLock};

const CHANNEL_BUFFER_SIZE: usize = 10000;

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
                                let auth_result = tokio::task::spawn_blocking(move || {
                                    WebsocketServer::run_authentication(
                                        &handler_clone,
                                        &scope_clone,
                                    )
                                })
                                .await
                                .unwrap_or(AuthResult::Failed);

                                if let AuthResult::Failed = auth_result {
                                    return (
                                        StatusCode::UNAUTHORIZED,
                                        format!("Authentication failed"),
                                    )
                                        .into_response();
                                }

                                let handler_clone = Arc::clone(handler);
                                let scope_clone = Python::attach(|py| scope.clone_ref(py));
                                let connect_result = tokio::task::spawn_blocking(move || {
                                    Python::attach(|py| -> PyResult<()> {
                                        handler_clone
                                            .borrow(py)
                                            .dispatch(py, &DispatchMethod::Connect(scope_clone))
                                    })
                                })
                                .await;

                                if let Ok(Err(e)) = connect_result {
                                    log::error!("Error in websocket connect callback: {}", e);
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        format!("Connection failed"),
                                    )
                                        .into_response();
                                }
                            }

                            let (response, fut) = ws.upgrade().unwrap();

                            tokio::spawn(async move {
                                if let Err(e) =
                                    WebsocketServer::handle_client(fut, group, path, state, scope)
                                        .await
                                {
                                    log::error!("Error in websocket connection: {}", e);
                                }
                            });

                            response.into_response()
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
        let path = uri.path().to_string();
        let query_string = uri.query().unwrap_or("").to_string();

        let mut headers = HashMap::new();
        for (name, value) in header_map.iter() {
            if let Ok(v) = value.to_str() {
                headers.insert(name.as_str().to_lowercase(), v.to_string());
            }
        }

        let mut cookies = HashMap::new();
        if let Some(cookie_header) = header_map.get("cookie") {
            if let Ok(cookie_str) = cookie_header.to_str() {
                for cookie in cookie_str.split(';') {
                    let cookie = cookie.trim();
                    if let Some((name, value)) = cookie.split_once('=') {
                        cookies.insert(name.trim().to_string(), value.trim().to_string());
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

    fn run_authentication(handler: &Py<SocketView>, scope: &Py<ConnectionScope>) -> AuthResult {
        Python::attach(|py| -> AuthResult {
            let view = handler.borrow(py);
            let auth_classes = &view.authentication_classes;

            if auth_classes.is_empty() {
                return AuthResult::Success;
            }

            for authenticator in auth_classes {
                let auth = authenticator
                    .getattr(py, "authenticate")
                    .expect(format!("There is no authenticate method, {}", authenticator).as_str());

                if let Some(res) = auth.call1(py, (scope,)).ok().filter(|res| !res.is_none(py)) {
                    scope.borrow_mut(py).user = Some(res);
                    return AuthResult::Success;
                }
            }

            AuthResult::Failed
        })
    }

    async fn handle_client(
        fut: upgrade::UpgradeFut,
        group: String,
        path: String,
        state: Arc<AppState>,
        scope: Py<ConnectionScope>,
    ) -> Result<(), WebSocketError> {
        let path_arc: Arc<str> = path.clone().into();

        let (reader, mut writer) = fut.await?.split(tokio::io::split);
        let mut reader = fastwebsockets::FragmentCollectorRead::new(reader);

        let (tx, mut rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let cid = state.channels.insert(group, tx);

        let mut send_task = tokio::spawn(async move {
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
        });

        let path_receive = Arc::clone(&path_arc);
        let state_receive = Arc::clone(&state);

        let mut receive_task = tokio::spawn(async move {
            loop {
                let frame = reader
                    .read_frame::<_, WebSocketError>(&mut |_| async { Ok(()) })
                    .await;

                match frame {
                    Ok(frame) => match frame.opcode {
                        OpCode::Close => {
                            if let Some(handler) = state_receive
                                .registry
                                .read()
                                .await
                                .get(path_receive.as_ref())
                                .cloned()
                            {
                                let scope_clone = Python::attach(|py| scope.clone_ref(py));
                                tokio::spawn(async move {
                                    if let Err(e) = Python::attach(|py| {
                                        let close_reason =
                                            String::from_utf8_lossy(&frame.payload).to_string();

                                        handler.borrow(py).dispatch(
                                            py,
                                            &DispatchMethod::Disconnect(
                                                scope_clone,
                                                Some((frame.opcode as u16, close_reason)),
                                            ),
                                        )
                                    }) {
                                        log::error!("Error in disconnect callback: {}", e);
                                    }
                                });
                            }
                            break;
                        }
                        OpCode::Text | OpCode::Binary => {
                            let text = String::from_utf8_lossy(&frame.payload).to_string();
                            let handler = state_receive
                                .registry
                                .read()
                                .await
                                .get(path_receive.as_ref())
                                .cloned();

                            if let Some(handler) = handler {
                                let scope_clone = Python::attach(|py| scope.clone_ref(py));
                                tokio::spawn(async move {
                                    if let Err(e) = Python::attach(|py| {
                                        handler.borrow(py).dispatch(
                                            py,
                                            &DispatchMethod::Receive(scope_clone, cid, text),
                                        )
                                    }) {
                                        log::error!("Error in receive callback: {}", e);
                                    }
                                });
                            }
                        }
                        _ => {}
                    },
                    Err(_) => break,
                }
            }
        });

        tokio::select! {
            _ = &mut send_task =>  receive_task.abort(),
            _ = &mut receive_task =>  send_task.abort(),
        }

        state.channels.remove(cid);
        Ok(())
    }
}

enum AuthResult {
    Success,
    Failed,
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

    fn send(&self, py: Python<'_>, cid: u64, msg: String) -> PyResult<()> {
        py.detach(|| {
            let state = Arc::clone(&self.state);
            tokio::spawn(async move {
                let _ = state
                    .channels
                    .send(cid, Arc::new(Message::Text(msg.into())))
                    .await;
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

    fn insert(&self, group: String, value: mpsc::Sender<Arc<Message>>) -> u64 {
        let ch_arc: Arc<mpsc::Sender<Arc<Message>>> = value.into();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        self.data.insert(id, Arc::clone(&ch_arc));
        self.grouped
            .entry(group)
            .or_insert_with(Vec::new)
            .push(Arc::clone(&ch_arc));

        id
    }

    fn remove(&self, id: u64) {
        self.data.remove(&id);
    }

    async fn send(&self, id: u64, msg: Arc<Message>) -> anyhow::Result<()> {
        if let Some(tx) = self.data.get(&id) {
            tx.send(msg).await?;
        };
        Ok(())
    }

    fn broadcast(&self, groups: &[String], msg: Arc<Message>) {
        let mut senders = Vec::new();

        for group in groups {
            if let Some(entry) = self.grouped.get(group) {
                senders.extend(entry.iter().cloned());
            }
        }

        tokio::spawn(async move {
            for tx in senders {
                if let Err(e) = tx.send(Arc::clone(&msg)).await {
                    log::warn!("Failed to send message to channel: {}", e);
                }
            }
        });
    }
}

#[pyclass(frozen)]
enum DispatchMethod {
    Connect(Py<ConnectionScope>),
    Receive(Py<ConnectionScope>, u64, String),
    Disconnect(Py<ConnectionScope>, Option<(u16, String)>),
}

#[pyclass]
struct SocketView {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    group: String,
    authentication_classes: Vec<Py<PyAny>>,
    connect_callback: Option<Py<PyFunction>>,
    receive_callback: Option<Py<PyFunction>>,
    disconnect_callback: Option<Py<PyFunction>>,
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
    fn connect(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.connect_callback = Some(func.clone_ref(py));
        func
    }

    fn receive(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.receive_callback = Some(func.clone_ref(py));
        func
    }

    fn disconnect(&mut self, py: Python<'_>, func: Py<PyFunction>) -> Py<PyFunction> {
        self.disconnect_callback = Some(func.clone_ref(py));
        func
    }

    fn dispatch(&self, py: Python<'_>, method: &DispatchMethod) -> PyResult<()> {
        match method {
            DispatchMethod::Connect(scope) => {
                if let Some(cb) = &self.connect_callback {
                    cb.call1(py, (scope,))?;
                }
            }
            DispatchMethod::Receive(scope, cid, data) => {
                if let Some(cb) = &self.receive_callback {
                    cb.call1(py, (scope, cid, data))?;
                }
            }
            DispatchMethod::Disconnect(scope, Some((code, reason))) => {
                if let Some(cb) = &self.disconnect_callback {
                    cb.call1(py, (scope, code, reason))?;
                }
            }
            DispatchMethod::Disconnect(scope, None) => {
                if let Some(cb) = &self.disconnect_callback {
                    cb.call1(py, (scope,))?;
                }
            }
        }
        Ok(())
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
