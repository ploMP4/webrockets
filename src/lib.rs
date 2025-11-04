use std::net::SocketAddr;

use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{CloseFrame, Utf8Bytes};
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

    if let Err(e) = socket
        .send(Message::Close(Some(CloseFrame {
            code: axum::extract::ws::close_code::NORMAL,
            reason: Utf8Bytes::from_static("Goodbye"),
        })))
        .await
    {
        tracing::warn!("Could not send Close due to {e}, probably it is ok?");
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
            let listener = tokio::net::TcpListener::bind("0.0.0.0:6969").await.unwrap();
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
fn django_rsws(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start_server, m)?)?;
    Ok(())
}
