use fastwebsockets::{upgrade, Frame, OpCode, WebSocketError};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use axum::extract::connect_info::ConnectInfo;
use axum::extract::Query;
use axum::{response::IntoResponse, routing::any, Router};
use pyo3::prelude::*;
use pyo3::types::PyFunction;
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, RwLock};

type Registry = RwLock<HashMap<String, Arc<Py<SocketView>>>>;

static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static REGISTRY: LazyLock<Registry> = LazyLock::new(|| RwLock::new(HashMap::new()));
static CHANNELS: LazyLock<ChannelStore> = LazyLock::new(|| ChannelStore::new());
static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("Unable to create tokio runtime"));

struct ChannelStore {
    next_id: AtomicU64,
    data: RwLock<HashMap<u64, mpsc::Sender<Message>>>,
}

impl ChannelStore {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            data: RwLock::new(HashMap::new()),
        }
    }

    async fn insert(&self, value: mpsc::Sender<Message>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.data.write().await.insert(id, value);
        id
    }

    async fn remove(&self, id: u64) {
        self.data.write().await.remove(&id);
    }

    async fn send(&self, id: u64, msg: Message) -> anyhow::Result<()> {
        if let Some(chan) = self.data.read().await.get(&id) {
            chan.send(msg).await?;
        };
        Ok(())
    }

    async fn broadcast(&self, msg: Message) -> anyhow::Result<()> {
        let map = self.data.read().await;
        for tx in map.values() {
            let _ = tx.send(msg.clone()).await?;
        }
        Ok(())
    }
}

#[pyclass]
enum DispatchMethod {
    Connect(),
    Receive(u64, String),
    Disconnect(Option<(u16, String)>),
}

#[pyclass]
#[allow(dead_code)]
struct SocketView {
    path: String,
    group: String,
    connect_callback: Option<Py<PyFunction>>,
    receive_callback: Option<Py<PyFunction>>,
    disconnect_callback: Option<Py<PyFunction>>,
}

#[pymethods]
impl SocketView {
    #[new]
    fn __new__(py: Python<'_>, path: String, group: String) -> PyResult<Py<Self>> {
        let instance = Py::new(
            py,
            Self {
                path: path,
                group: group.clone(),
                connect_callback: None,
                receive_callback: None,
                disconnect_callback: None,
            },
        )?;

        let instance_ref = instance.clone_ref(py);
        py.detach(|| {
            RUNTIME.block_on(async {
                REGISTRY.write().await.insert(group, Arc::new(instance_ref));
            });
        });

        Ok(instance)
    }

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

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum Message {
    Text(Arc<str>, Arc<str>),
    Binary(Arc<str>, Arc<[u8]>),
    Close(Arc<str>),
}

#[derive(Deserialize)]
struct Params {
    group: String,
}

async fn fast_handle_client(
    fut: upgrade::UpgradeFut,
    who: SocketAddr,
    group: String,
) -> Result<(), WebSocketError> {
    let group_arc: Arc<str> = group.clone().into();
    let (reader, mut writer) = fut.await?.split(tokio::io::split);
    let mut reader = fastwebsockets::FragmentCollectorRead::new(reader);

    let (tx, mut rx) = mpsc::channel(100000);
    let cid = CHANNELS.insert(tx).await;

    let group_send = Arc::clone(&group_arc);

    let mut send_task = RUNTIME.spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
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

    let mut receive_task = RUNTIME.spawn(async move {
        loop {
            let frame = reader
                .read_frame::<_, WebSocketError>(&mut |_| async { Ok(()) })
                .await;

            match frame {
                Ok(frame) => match frame.opcode {
                    OpCode::Close => break,
                    OpCode::Text | OpCode::Binary => {
                        let text = String::from_utf8_lossy(&frame.payload).to_string();
                        let handler = REGISTRY.read().await.get(group_receive.as_ref()).cloned();

                        if let Some(handler) = handler {
                            RUNTIME.spawn(async move {
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
    CHANNELS.remove(cid).await;
    Ok(())
}

async fn fast_ws_handler(
    ws: upgrade::IncomingUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(params): Query<Params>,
) -> impl IntoResponse {
    let (response, fut) = ws.upgrade().unwrap();

    RUNTIME.spawn(async move {
        if let Err(e) = fast_handle_client(fut, addr, params.group).await {
            tracing::error!("Error in websocket connection: {}", e);
        }
    });

    response
}

#[pyfunction]
fn run_server(py: Python<'_>) -> PyResult<()> {
    py.detach(|| {
        if SERVER_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            tracing::info!("Server already started, skipping...");
            return;
        }

        RUNTIME.block_on(async {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .init();

            let app = Router::new().route("/ws", any(fast_ws_handler));

            let listener = tokio::net::TcpListener::bind("127.0.0.1:6969")
                .await
                .unwrap();

            tracing::debug!("listening on {}", listener.local_addr().unwrap());

            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                SERVER_STARTED.swap(false, std::sync::atomic::Ordering::SeqCst);
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to install Ctrl-C handler");

                tracing::info!("Exit signal received, shutting down...");
            })
            .await
            .unwrap();
        });
    });

    Ok(())
}

#[pyclass]
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
    #[pymodule_export]
    use super::log;
    #[pymodule_export]
    use super::run_server;
    #[pymodule_export]
    use super::LogLevel;
    #[pymodule_export]
    use super::SocketView;

    #[pymodule_export]
    use super::broadcast_text;
}

// #[pyfunction]
// fn send(cid: u64, msg: String) -> PyResult<()> {
//     RUNTIME.block_on(async {
//         CHANNELS.send(cid, msg);
//     });
//     Ok(())
// }

#[pyfunction]
fn broadcast_text(py: Python<'_>, groups: Vec<String>, msg: String) -> PyResult<()> {
    py.detach(|| {
        let msg: Arc<str> = msg.into();
        for group in groups {
            let group_arc: Arc<str> = group.clone().into();
            RUNTIME
                .spawn(CHANNELS.broadcast(Message::Text(Arc::clone(&group_arc), Arc::clone(&msg))));
        }
    });
    Ok(())
}
