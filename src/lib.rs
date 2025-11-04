use std::net::SocketAddr;
use std::ops::ControlFlow;

use axum::extract::connect_info::ConnectInfo;
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
use pyo3::prelude::*;
use tokio::runtime::Runtime;

fn process_message(msg: Message, who: SocketAddr) -> ControlFlow<(), ()> {
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

        Message::Pong(v) => {
            println!(">>> {who} sent pong with {v:?}");
        }
        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>> {who} sent ping with {v:?}");
        }
    }
    ControlFlow::Continue(())
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {
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

    loop {
        if let Some(msg) = socket.recv().await {
            if let Ok(msg) = msg {
                if process_message(msg, who).is_break() {
                    break;
                }

                if socket
                    .send(Message::Text(format!("Hi times!").into()))
                    .await
                    .is_err()
                {
                    println!("client {who} abruptly disconnected");
                    break;
                }
            } else {
                println!("client {who} abruptly disconnected");
                break;
            }
        }
    }

    tracing::info!("Websocket context {who} destroyed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

#[pyfunction]
fn start_server() -> PyResult<()> {
    std::thread::spawn(move || {
        let rt = Runtime::new().expect("Unable to create tokio runtime");
        rt.block_on(async move {
            let app = Router::new().route("/ws", any(ws_handler));
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

#[pymodule]
fn django_wsrs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start_server, m)?)?;
    Ok(())
}
