use fastwebsockets::{upgrade, Frame, OpCode, WebSocketError};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock, OnceLock};

use axum::extract::connect_info::ConnectInfo;
use axum::extract::Query;
use axum::{response::IntoResponse, routing::any, Router};
use pyo3::prelude::*;
use pyo3::types::PyFunction;
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::{broadcast, RwLock};

type Registry = Arc<RwLock<HashMap<String, Arc<Py<SocketView>>>>>;

static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static REGISTRY: LazyLock<Registry> = LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));
static BROADCAST_TX: OnceLock<broadcast::Sender<BroadcastMessage>> = OnceLock::new();
static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("Unable to create tokio runtime"));

#[pyclass]
enum DispatchMethod {
    Connect(),
    Receive(String),
    Disconnect(Option<(u16, String)>),
}

#[pyclass]
#[allow(dead_code)]
struct SocketView {
    group: String,
    connect_callback: Option<Py<PyFunction>>,
    receive_callback: Option<Py<PyFunction>>,
    disconnect_callback: Option<Py<PyFunction>>,
}

#[pymethods]
impl SocketView {
    #[new]
    fn __new__(group: String, py: Python<'_>) -> PyResult<Py<Self>> {
        let instance = Py::new(
            py,
            Self {
                group: group.clone(),
                connect_callback: None,
                receive_callback: None,
                disconnect_callback: None,
            },
        )?;

        RUNTIME.block_on(async {
            REGISTRY
                .write()
                .await
                .insert(group, Arc::new(instance.clone_ref(py)));
        });

        Ok(instance)
    }

    fn connect(&mut self, py: Python<'_>, func: Py<PyFunction>) -> PyResult<Py<PyFunction>> {
        self.connect_callback = Some(func.clone_ref(py));
        Ok(func)
    }

    fn receive(&mut self, py: Python<'_>, func: Py<PyFunction>) -> PyResult<Py<PyFunction>> {
        self.receive_callback = Some(func.clone_ref(py));
        Ok(func)
    }

    fn disconnect(&mut self, py: Python<'_>, func: Py<PyFunction>) -> PyResult<Py<PyFunction>> {
        self.disconnect_callback = Some(func.clone_ref(py));
        Ok(func)
    }

    fn dispatch(&self, py: Python<'_>, method: &DispatchMethod) -> PyResult<()> {
        match method {
            DispatchMethod::Connect() => {
                if let Some(cb) = &self.connect_callback {
                    cb.call0(py)?;
                }
            }
            DispatchMethod::Receive(data) => {
                if let Some(cb) = &self.receive_callback {
                    cb.call1(py, (data,))?;
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
enum BroadcastMessage {
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
    let ws = fut.await?;
    let (reader, mut writer) = ws.split(tokio::io::split);
    let mut reader = fastwebsockets::FragmentCollectorRead::new(reader);

    let broadcast_group = Arc::clone(&group_arc);

    let mut broadcast_task = tokio::spawn(async move {
        let mut rx = BROADCAST_TX.get().unwrap().subscribe();

        while let Ok(msg) = rx.recv().await {
            match msg {
                BroadcastMessage::Text(group, text)
                    if group.as_ref() == broadcast_group.as_ref() =>
                {
                    let frame = Frame::text(fastwebsockets::Payload::Borrowed(text.as_bytes()));
                    if writer.write_frame(frame).await.is_err() {
                        break;
                    }
                }
                BroadcastMessage::Binary(group, data)
                    if group.as_ref() == broadcast_group.as_ref() =>
                {
                    let frame = Frame::binary(fastwebsockets::Payload::Borrowed(&data));
                    if writer.write_frame(frame).await.is_err() {
                        break;
                    }
                }
                BroadcastMessage::Close(group_name)
                    if group_name.as_ref() == broadcast_group.as_ref() =>
                {
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

    let mut receive_task = tokio::spawn(async move {
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
                            tokio::task::spawn_blocking(move || {
                                Python::with_gil(|py| -> PyResult<()> {
                                    handler
                                        .borrow(py)
                                        .dispatch(py, &DispatchMethod::Receive(text.to_string()))
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
        _ = &mut broadcast_task => {
            receive_task.abort();
        }
        _ = &mut receive_task => {
            broadcast_task.abort();
        }
    }

    tracing::info!("Websocket context {who} destroyed");
    Ok(())
}

async fn fast_ws_handler(
    ws: upgrade::IncomingUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(params): Query<Params>,
) -> impl IntoResponse {
    let (response, fut) = ws.upgrade().unwrap();

    tokio::spawn(async move {
        if let Err(e) = fast_handle_client(fut, addr, params.group).await {
            eprintln!("Error in websocket connection: {}", e);
        }
    });

    response
}

#[pyfunction]
fn start_server() -> PyResult<()> {
    if SERVER_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        println!("Server already started, skipping...");
        return Ok(());
    }

    std::thread::spawn(move || {
        let (tx, _rx) = broadcast::channel::<BroadcastMessage>(100000);

        BROADCAST_TX.get_or_init(|| tx.clone());

        RUNTIME.block_on(async {
            let app = Router::new().route("/ws", any(fast_ws_handler));

            let listener = tokio::net::TcpListener::bind("127.0.0.1:6969")
                .await
                .unwrap();

            tracing::debug!("listening on {}", listener.local_addr().unwrap());

            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
    });
    Ok(())
}

#[pyfunction]
fn broadcast_text(groups: Vec<String>, msg: String) -> PyResult<HashMap<String, usize>> {
    let tx = BROADCAST_TX
        .get()
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Server not started"))?;

    let msg_arc: Arc<str> = msg.into();
    let mut receiver_counts = HashMap::with_capacity(groups.len());

    for group in groups {
        let group_arc: Arc<str> = group.clone().into();
        let receiver_count = tx
            .send(BroadcastMessage::Text(group_arc, Arc::clone(&msg_arc)))
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Broadcast failed: {}",
                    e
                ))
            })?;

        receiver_counts.insert(group, receiver_count);
    }

    Ok(receiver_counts)
}

#[pymodule]
mod django_wsrs {
    #[pymodule_export]
    use super::SocketView;

    #[pymodule_export]
    use super::start_server;

    #[pymodule_export]
    use super::broadcast_text;
}
