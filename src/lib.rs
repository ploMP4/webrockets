use fastwebsockets::{upgrade, Frame, OpCode, WebSocketError};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Query, State};
use axum::{response::IntoResponse, routing::get, Router};
use pyo3::prelude::*;
use pyo3::types::PyFunction;
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, RwLock};

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

#[derive(Deserialize)]
struct Params {
    group: String,
}

#[derive(Debug)]
#[allow(dead_code)]
enum Message {
    Text(Arc<str>, Arc<str>),
    Binary(Arc<str>, Arc<[u8]>),
    Close(Arc<str>),
}

#[pyclass(frozen)]
struct WebsocketServer {
    rt: Runtime,
    state: Arc<AppState>,
}

impl WebsocketServer {
    fn new() -> Self {
        Self {
            rt: Runtime::new().expect("Unable to create tokio runtime"),
            state: Arc::new(AppState::new()),
        }
    }

    fn runserver(&self) {
        self.rt.block_on(async {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .init();

            let app = Router::new()
                .route("/ws", get(WebsocketServer::handler))
                .with_state(Arc::clone(&self.state));

            let listener = tokio::net::TcpListener::bind("127.0.0.1:6969")
                .await
                .unwrap();

            tracing::debug!("listening on {}", listener.local_addr().unwrap());

            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to install Ctrl-C handler");

                tracing::info!("Exit signal received, shutting down...");
            })
            .await
            .unwrap();
        });
    }

    async fn handler(
        ws: upgrade::IncomingUpgrade,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        Query(params): Query<Params>,
        State(state): State<Arc<AppState>>,
    ) -> impl IntoResponse {
        let (response, fut) = ws.upgrade().unwrap();

        tokio::spawn(async move {
            if let Err(e) = WebsocketServer::handle_client(fut, addr, params.group, state).await {
                tracing::error!("Error in websocket connection: {}", e);
            }
        });

        response
    }

    async fn handle_client(
        fut: upgrade::UpgradeFut,
        who: SocketAddr,
        group: String,
        state: Arc<AppState>,
    ) -> Result<(), WebSocketError> {
        let group_arc: Arc<str> = group.clone().into();
        let (reader, mut writer) = fut.await?.split(tokio::io::split);
        let mut reader = fastwebsockets::FragmentCollectorRead::new(reader);

        let (tx, mut rx) = mpsc::channel(100000);
        let cid = state.channels.insert(tx).await;

        let group_send = Arc::clone(&group_arc);

        let mut send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg.as_ref() {
                    Message::Text(group, text) if group.as_ref() == group_send.as_ref() => {
                        let frame = Frame::text(fastwebsockets::Payload::Borrowed(text.as_bytes()));
                        if writer.write_frame(frame).await.is_err() {
                            break;
                        }
                    }
                    Message::Binary(group, data) if group.as_ref() == group_send.as_ref() => {
                        let frame = Frame::binary(fastwebsockets::Payload::Borrowed(&data));
                        if writer.write_frame(frame).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(group) if group.as_ref() == group_send.as_ref() => {
                        // let frame = Frame::close(code, reason);
                        // if tx.write_frame(frame).await.is_err() {
                        //     break;
                        // }
                        break;
                    }
                    _ => continue,
                }
            }
        });

        let group_receive = Arc::clone(&group_arc);
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
                                .get(group_arc.as_ref())
                                .cloned()
                            {
                                tokio::spawn(async move {
                                    Python::attach(|py| {
                                        handler
                                            .borrow(py)
                                            .dispatch(py, &DispatchMethod::Disconnect(None)) // TODO:
                                            .ok();
                                    });
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
                                .get(group_receive.as_ref())
                                .cloned();

                            if let Some(handler) = handler {
                                tokio::spawn(async move {
                                    Python::attach(|py| -> PyResult<()> {
                                        handler.borrow(py).dispatch(
                                            py,
                                            &DispatchMethod::Receive(cid, text.to_string()),
                                        )
                                    })
                                    .ok();
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
            _ = &mut send_task => {
                receive_task.abort();
            }
            _ = &mut receive_task => {
                send_task.abort();
            }
        }

        tracing::info!("Websocket context {who} destroyed");
        state.channels.remove(cid).await;
        Ok(())
    }
}

#[pymethods]
impl WebsocketServer {
    fn start(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.runserver());
        Ok(())
    }

    fn __call__(&self, py: Python<'_>, path: String, group: String) -> PyResult<Py<SocketView>> {
        let instance = Py::new(py, SocketView::new(path, group.clone()))?;
        let instance_ref = instance.clone_ref(py);
        py.detach(|| {
            self.rt.block_on(async {
                self.state
                    .registry
                    .write()
                    .await
                    .insert(group, Arc::new(instance_ref))
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
                    .send(cid, Arc::new(Message::Text("chat".into(), msg.into())))
                    .await;
            });
        });
        Ok(())
    }

    fn broadcast_text(&self, py: Python<'_>, groups: Vec<String>, msg: String) {
        py.detach(|| {
            let msg: Arc<str> = msg.into();
            for group in groups {
                let state = Arc::clone(&self.state);
                let msg = Arc::clone(&msg);
                tokio::spawn(async move {
                    state
                        .channels
                        .broadcast(Arc::new(Message::Text(group.into(), msg)))
                        .await
                });
            }
        });
    }
}

struct ChannelStore {
    next_id: AtomicU64,
    data: RwLock<HashMap<u64, mpsc::Sender<Arc<Message>>>>,
}

impl ChannelStore {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            data: RwLock::new(HashMap::new()),
        }
    }

    async fn insert(&self, value: mpsc::Sender<Arc<Message>>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.data.write().await.insert(id, value);
        id
    }

    async fn remove(&self, id: u64) {
        self.data.write().await.remove(&id);
    }

    async fn send(&self, id: u64, msg: Arc<Message>) -> anyhow::Result<()> {
        if let Some(tx) = self.data.read().await.get(&id) {
            tx.send(msg).await?;
        };
        Ok(())
    }

    async fn broadcast(&self, msg: Arc<Message>) -> anyhow::Result<()> {
        let map = self.data.read().await;
        for tx in map.values() {
            let _ = tx.send(Arc::clone(&msg)).await?;
        }
        Ok(())
    }
}

#[pyclass(frozen)]
enum DispatchMethod {
    Connect(),
    Receive(u64, String),
    Disconnect(Option<(u16, String)>),
}

#[pyclass]
struct SocketView {
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    group: String,
    connect_callback: Option<Py<PyFunction>>,
    receive_callback: Option<Py<PyFunction>>,
    disconnect_callback: Option<Py<PyFunction>>,
}

impl SocketView {
    fn new(path: String, group: String) -> Self {
        Self {
            path: path,
            group: group,
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
            DispatchMethod::Connect() => {
                if let Some(cb) = &self.connect_callback {
                    cb.call0(py)?;
                }
            }
            DispatchMethod::Receive(cid, data) => {
                if let Some(cb) = &self.receive_callback {
                    cb.call1(py, (cid, data))?;
                }
            }
            DispatchMethod::Disconnect(Some((code, reason))) => {
                if let Some(cb) = &self.disconnect_callback {
                    cb.call1(py, (code, reason))?;
                }
            }
            DispatchMethod::Disconnect(None) => {
                if let Some(cb) = &self.disconnect_callback {
                    cb.call0(py)?;
                }
            }
        }
        Ok(())
    }
}

#[pyclass(frozen)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum LogLevel {
    DEBUG,
    INFO,
    WARN,
    ERROR,
}

#[pyfunction]
fn log(py: Python<'_>, level: LogLevel, msg: &str) {
    py.detach(|| match level {
        LogLevel::DEBUG => tracing::debug!(msg),
        LogLevel::INFO => tracing::info!(msg),
        LogLevel::WARN => tracing::warn!(msg),
        LogLevel::ERROR => tracing::error!(msg),
    });
}

#[pymodule]
mod django_wsrs {
    use pyo3::prelude::*;

    #[pymodule_export]
    use super::log;
    #[pymodule_export]
    use super::LogLevel;
    #[pymodule_export]
    use super::SocketView;
    #[pymodule_export]
    use super::WebsocketServer;

    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add("Websocket", WebsocketServer::new())?;
        Ok(())
    }
}
