use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Query, State};
use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::any,
    Router,
};
use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use pyo3::types::{PyCFunction, PyDict, PyTuple, PyType};
use pyo3::{intern, prelude::*};
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;

static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static BROADCAST_TX: OnceLock<broadcast::Sender<BroadcastMessage>> = OnceLock::new();
static REGISTRY: OnceLock<Arc<Mutex<HashMap<String, Py<SocketView>>>>> = OnceLock::new();

fn get_registry() -> Arc<Mutex<HashMap<String, Py<SocketView>>>> {
    REGISTRY
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

#[pyclass]
enum DispatchMethod {
    Connect(),
    Receive(String),
    Disconnect(Option<(u16, String)>),
}

#[pyclass(subclass)]
#[derive(Clone)]
#[allow(dead_code)]
struct SocketView {
    pub group: String,
}

#[pymethods]
impl SocketView {
    #[new]
    fn __new__(group: String) -> Self {
        Self { group: group }
    }

    pub fn connect(&self) -> PyResult<()> {
        println!("Hello there connect");
        Ok(())
    }

    pub fn receive(&self, data: String) -> PyResult<()> {
        println!("Hello there receive {}", data);
        Ok(())
    }

    pub fn disconnect(&self, code: Option<u16>, reason: Option<String>) -> PyResult<()> {
        if let (Some(code), Some(reason)) = (code, reason) {
            println!("Hello there disconnect {}: {}", code, reason);
        }
        Ok(())
    }

    #[classmethod]
    fn as_view<'a>(cls: &Bound<'a, PyType>) -> PyResult<Bound<'a, PyCFunction>> {
        let module = Python::import(cls.py(), "django.http")?;
        let http_request_class = module.getattr("HttpResponse")?.unbind();

        let group: String = cls.getattr("group")?.extract()?;

        let url = format!("http://localhost:6969/ws?group={}", group);
        let instance: Bound<SocketView> = cls.call1((group.clone(),))?.extract()?;

        let registry = get_registry();
        registry.lock().unwrap().insert(group, instance.into());
        drop(registry);

        let view = move |args: &Bound<'_, PyTuple>,
                         _kwargs: Option<&Bound<'_, PyDict>>|
              -> PyResult<PyObject> {
            let py = args.py();

            let http_request_class = http_request_class.bind(py);

            let kwargs = PyDict::new(py);
            kwargs.set_item("status", 307)?;

            let instance = http_request_class.call((), Some(&kwargs))?;
            instance.set_item("Location", url.clone())?;

            Ok(instance.into())
        };

        let view = PyCFunction::new_closure(cls.py(), None, None, view)?;
        view.setattr(intern!(view.py(), "__module__"), "django_wsrs")?;

        Ok(view)
    }

    fn dispatch(slf: PyRef<'_, Self>, method: &DispatchMethod) -> PyResult<()> {
        let py = slf.py();
        let obj = slf.into_pyobject(py)?;
        match method {
            DispatchMethod::Connect() => obj.call_method0("connect"),
            DispatchMethod::Receive(data) => obj.call_method1("receive", (data,)),
            DispatchMethod::Disconnect(Some((code, reason))) => {
                obj.call_method1("disconnect", (code, reason))
            }
            DispatchMethod::Disconnect(None) => obj.call_method0("disconnect"),
        }?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum BroadcastMessage {
    Text(String, String),
    Binary(String, Vec<u8>),
    Close(String),
}

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<BroadcastMessage>,
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr, group: String, state: AppState) {
    if socket
        .send(Message::Ping(Bytes::from_static(&[1, 2, 3])))
        .await
        .is_ok()
    {
        tracing::info!("Pinged {who}...");
    } else {
        tracing::error!("Could not send ping {who}!");
        return;
    }

    let mut rx = state.tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    let bgroup = group.clone();
    let mut broadcast_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                BroadcastMessage::Text(group_name, text) if group_name == bgroup => {
                    if sender.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                BroadcastMessage::Binary(group_name, data) if group_name == bgroup => {
                    if sender.send(Message::Binary(data.into())).await.is_err() {
                        break;
                    }
                }
                BroadcastMessage::Close(group_name) if group_name == bgroup => {
                    let _ = sender.send(Message::Close(None)).await;
                    break;
                }
                _ => {}
            }
        }
    });

    let mut receive_task = tokio::spawn(async move {
        loop {
            if let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(t)) => {
                        let _ = Python::with_gil(|py| -> PyResult<()> {
                            let registry = get_registry();
                            if let Some(handler) = registry.lock().unwrap().get(&group) {
                                handler.bind(py).call_method1(
                                    "dispatch",
                                    (DispatchMethod::Receive(t.to_string()),),
                                )?;
                            }
                            Ok(())
                        });
                    }
                    Ok(Message::Binary(b)) => {
                        // let _ = Python::with_gil(|py| -> PyResult<()> {
                        //     let registry = get_registry();
                        //     if let Some(handler) = registry.lock().unwrap().get("chat") {
                        //         handler.bind(py).call_method1(
                        //             "dispatch",
                        //             (DispatchMethod::Receive(b.str),),
                        //         )?;
                        //     }
                        //     Ok(())
                        // });
                    }
                    Ok(Message::Close(c)) => {
                        let mut dc_data: Option<(u16, String)> = None;
                        if let Some(cf) = c {
                            dc_data = Some((cf.code, cf.reason.as_str().to_string()));
                        }

                        let _ = Python::with_gil(|py| -> PyResult<()> {
                            let registry = get_registry();
                            if let Some(handler) = registry.lock().unwrap().get(&group) {
                                handler.bind(py).call_method1(
                                    "dispatch",
                                    (DispatchMethod::Disconnect(dc_data),),
                                )?;
                            }
                            Ok(())
                        });
                        break;
                    }
                    Err(e) => {
                        println!("client {who} abruptly disconnected {e}");
                        break;
                    }
                    _ => {}
                }
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
}

#[derive(Deserialize)]
struct Params {
    group: String,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(params): Query<Params>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let _ = Python::with_gil(|py| -> PyResult<()> {
        let registry = get_registry();
        if let Some(handler) = registry.lock().unwrap().get("chat") {
            handler
                .bind(py)
                .call_method1("dispatch", (DispatchMethod::Connect(),))?;
        }
        Ok(())
    });
    ws.on_upgrade(move |socket| handle_socket(socket, addr, params.group, state))
}

#[pyfunction]
fn start_server() -> PyResult<()> {
    if SERVER_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        println!("Server already started, skipping...");
        return Ok(());
    }

    std::thread::spawn(move || {
        let rt = Runtime::new().expect("Unable to create tokio runtime");
        let (tx, _rx) = broadcast::channel::<BroadcastMessage>(10000);

        BROADCAST_TX.get_or_init(|| tx.clone());

        let app_state = AppState { tx: tx.clone() };
        rt.block_on(async move {
            let app = Router::new()
                .route("/ws", any(ws_handler))
                .with_state(app_state);

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

    let mut receiver_counts = HashMap::new();

    for group in groups {
        let receiver_count = tx
            .send(BroadcastMessage::Text(group.clone(), msg.clone()))
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
fn django_wsrs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SocketView>()?;
    m.add_function(wrap_pyfunction!(start_server, m)?)?;
    m.add_function(wrap_pyfunction!(broadcast_text, m)?)?;
    Ok(())
}
