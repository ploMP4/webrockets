use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::sync::atomic::AtomicBool;

use axum::extract::connect_info::ConnectInfo;
use axum::extract::State;
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
use pyo3::prelude::*;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;

static BROADCAST_TX: std::sync::OnceLock<broadcast::Sender<BroadcastMessage>> =
    std::sync::OnceLock::new();

static SERVER_STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
enum BroadcastMessage {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<BroadcastMessage>,
}

fn process_message(msg: &Message, who: SocketAddr) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            println!(">>> {who} sent str: {t:?}");
        }
        Message::Binary(d) => {
            println!(">>> {who} sent {} bytes: {d:?}", d.len());
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> {who} sent close with code {} and reason `{}`",
                    cf.code, cf.reason
                );
            } else {
                println!(">>> {who} somehow sent close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }
        _ => {}
    }
    ControlFlow::Continue(())
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr, state: AppState) {
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

    let mut broadcast_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                BroadcastMessage::Text(text) => {
                    if sender.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                BroadcastMessage::Binary(data) => {
                    if sender.send(Message::Binary(data.into())).await.is_err() {
                        break;
                    }
                }
                BroadcastMessage::Close => {
                    let _ = sender.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    });

    let tx = state.tx.clone();
    let mut receive_task = tokio::spawn(async move {
        loop {
            if let Some(msg) = receiver.next().await {
                if let Ok(msg) = msg {
                    if process_message(&msg, who).is_break() {
                        break;
                    }

                    if tx
                        .send(BroadcastMessage::Text("Hello".to_string()))
                        .is_err()
                    {
                        println!("channel send failed");
                        break;
                    }
                } else {
                    println!("client {who} abruptly disconnected");
                    break;
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

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
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
fn broadcast_text(msg: String) -> PyResult<usize> {
    let tx = BROADCAST_TX
        .get()
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Server not started"))?;

    let receiver_count = tx.send(BroadcastMessage::Text(msg)).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Broadcast failed: {}", e))
    })?;

    Ok(receiver_count)
}

#[pymodule]
fn django_wsrs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start_server, m)?)?;
    m.add_function(wrap_pyfunction!(broadcast_text, m)?)?;
    Ok(())
}
