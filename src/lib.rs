use fastwebsockets::{upgrade, Frame, OpCode, WebSocketError};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};

use axum::extract::connect_info::ConnectInfo;
use axum::extract::Query;
use axum::{response::IntoResponse, routing::any, Router};
use pyo3::types::{PyCFunction, PyDict, PyTuple, PyType};
use pyo3::{intern, prelude::*};
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::{broadcast, RwLock};

type Registry = Arc<RwLock<HashMap<String, Arc<Py<SocketView>>>>>;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static SERVER_STARTED: AtomicBool = AtomicBool::new(false);
static BROADCAST_TX: OnceLock<broadcast::Sender<BroadcastMessage>> = OnceLock::new();
static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn get_registry() -> Registry {
    REGISTRY
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
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
        // println!("Hello there connect");
        Ok(())
    }

    pub fn receive(&self, data: String) -> PyResult<()> {
        // println!("Hello there receive {}", data);
        Ok(())
    }

    pub fn disconnect(&self, code: Option<u16>, reason: Option<String>) -> PyResult<()> {
        // if let (Some(code), Some(reason)) = (code, reason) {
        //     println!("Hello there disconnect {}: {}", code, reason);
        // }
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
        let rt = RUNTIME.get_or_init(|| Runtime::new().expect("Unable to create tokio runtime"));
        rt.block_on(async {
            registry
                .write()
                .await
                .insert(group, Arc::new(instance.into()));
        });

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
                        let handler = get_registry()
                            .read()
                            .await
                            .get(group_receive.as_ref())
                            .cloned();

                        if let Some(handler) = handler {
                            tokio::task::spawn_blocking(move || {
                                Python::with_gil(|py| -> PyResult<()> {
                                    handler.bind(py).call_method1(
                                        "dispatch",
                                        (DispatchMethod::Receive(text.to_string()),),
                                    )?;
                                    Ok(())
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

    let rt = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .build()
            .expect("Unable to create tokio runtime")
    });

    std::thread::spawn(move || {
        let (tx, _rx) = broadcast::channel::<BroadcastMessage>(100000);

        BROADCAST_TX.get_or_init(|| tx.clone());

        rt.block_on(async {
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
fn django_wsrs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SocketView>()?;
    m.add_function(wrap_pyfunction!(start_server, m)?)?;
    m.add_function(wrap_pyfunction!(broadcast_text, m)?)?;
    Ok(())
}
