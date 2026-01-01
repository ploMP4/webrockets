use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{routing::get, Router};
use fastwebsockets::{upgrade, Frame, OpCode, WebSocketError};
use pyo3::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, watch, RwLock};

use crate::callback::Callback;
use crate::channel_store::ChannelStore;
use crate::connection::{Connection, IncomingConnection};
use crate::socket_view::SocketView;
use crate::{start_python_event_loop, Message};

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

#[pyclass(frozen)]
pub(crate) struct WebsocketServer {
    rt: Runtime,
    state: Arc<AppState>,
    context: RwLock<Vec<(String, String)>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl WebsocketServer {
    pub(crate) fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            rt: Runtime::new().expect("Unable to create tokio runtime"),
            state: Arc::new(AppState::new()),
            context: RwLock::new(Vec::new()),
            shutdown_tx,
            shutdown_rx,
        }
    }

    fn runserver(&self, host: &str, port: &str) {
        let mut shutdown_rx = self.shutdown_rx.clone();

        self.rt.block_on(async {
            let mut app = Router::new();
            for (group, path) in self.context.read().await.clone() {
                let route_path = format!("/{path}");
                app = app.route(
                    &route_path,
                    get(move |uri, headers, ws, State(state)| {
                        WebsocketServer::handle_ws_upgrade(uri, headers, ws, state, path, group)
                    }),
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

    fn authenticated(
        py: Python<'_>,
        auth_classes: &Vec<Py<PyAny>>,
        conn: &Py<IncomingConnection>,
    ) -> bool {
        if auth_classes.is_empty() {
            return true;
        }

        for authenticator in auth_classes {
            if let Ok(auth) = authenticator.getattr(py, "authenticate") {
                if let Some(res) = auth.call1(py, (conn,)).ok().filter(|res| !res.is_none(py)) {
                    conn.borrow_mut(py).as_super().user = Some(res);
                    return true;
                }
            }
        }
        false
    }

    async fn handle_ws_upgrade(
        uri: Uri,
        headers: HeaderMap,
        ws: upgrade::IncomingUpgrade,
        state: Arc<AppState>,
        path: String,
        group: String,
    ) -> Response {
        let conn = IncomingConnection::py_new(&uri, &headers);

        let Some(handler) = state.registry.read().await.get(&path).cloned() else {
            return (StatusCode::NOT_FOUND, "Handler not found").into_response();
        };

        let conn = match WebsocketServer::authenticate_and_connect(handler.clone(), conn).await {
            Ok(conn) => conn,
            Err(response) => return response,
        };

        let (response, fut) = match ws.upgrade() {
            Ok(upgrade) => upgrade,
            Err(_) => return (StatusCode::BAD_REQUEST, "WebSocket upgrade failed").into_response(),
        };

        tokio::spawn(async move {
            if let Err(e) = WebsocketServer::handle_client(fut, group, state, conn, handler).await {
                log::error!("Error in websocket connection: {}", e);
            }
        });

        response.into_response()
    }

    async fn authenticate_and_connect(
        handler: Arc<Py<SocketView>>,
        conn: Py<IncomingConnection>,
    ) -> Result<Py<Connection>, Response> {
        let result = tokio::task::spawn_blocking(move || {
            Python::attach(|py| -> Result<Py<Connection>, (StatusCode, &'static str)> {
                let view = handler.borrow(py);

                if !WebsocketServer::authenticated(py, &view.authentication_classes, &conn) {
                    return Err((StatusCode::UNAUTHORIZED, "Authentication failed"));
                }

                if let Some(cb) = &view.connect_callback {
                    if let Err(e) = cb.invoke(py, (&conn,)) {
                        log::error!("Error in websocket connect callback: {}", e);
                        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Connection failed"));
                    }
                }

                IncomingConnection::upgrade(&conn, py).map_err(|e| {
                    log::error!("Error upgrading connection: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Connection upgrade failed",
                    )
                })
            })
        })
        .await;

        match result {
            Ok(Ok(conn)) => Ok(conn),
            Ok(Err((status, msg))) => Err((status, msg).into_response()),
            Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()),
        }
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
        conn: Py<Connection>,
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
                                Ok(s) => cb.invoke(py, (&conn, close_code, s)),
                                Err(_) => {
                                    let s = String::from_utf8_lossy(payload_bytes);
                                    cb.invoke(py, (&conn, close_code, s))
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
                                Ok(s) => cb.invoke(py, (&conn, s)),
                                Err(_) => {
                                    let s = String::from_utf8_lossy(payload_bytes);
                                    cb.invoke(py, (&conn, s))
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
        conn: Py<Connection>,
        handler: Arc<Py<SocketView>>,
    ) -> Result<(), WebSocketError> {
        let (reader, writer) = fut.await?.split(tokio::io::split);
        let reader = fastwebsockets::FragmentCollectorRead::new(reader);

        let (tx, rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let tx_arc: Arc<mpsc::Sender<Arc<Message>>> = Arc::new(tx);
        let channel_id = state.channels.register(&group, tx_arc.clone());

        let (receive_cb, disconnect_cb) = Python::attach(|py| {
            conn.borrow_mut(py).sender = Some(tx_arc);

            let view = handler.borrow(py);
            (
                view.receive_callback.as_ref().map(|cb| cb.clone_ref(py)),
                view.disconnect_callback.as_ref().map(|cb| cb.clone_ref(py)),
            )
        });

        let mut send_task = tokio::spawn(WebsocketServer::send_listener(rx, writer));

        let mut receive_task = tokio::spawn(WebsocketServer::receive_listener(
            reader,
            conn,
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
        &self,
        py: Python<'_>,
        path: String,
        group: String,
        authentication_classes: Option<Vec<Py<PyAny>>>,
    ) -> PyResult<Py<SocketView>> {
        self.context
            .blocking_write()
            .push((group.clone(), path.clone()));

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

    fn broadcast_text(&self, py: Python<'_>, groups: Vec<String>, msg: String) {
        py.detach(|| {
            self.state
                .channels
                .broadcast(&groups, Arc::new(Message::Text(msg.into())));
        });
    }
}
