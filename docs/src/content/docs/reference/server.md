---
title: WebsocketServer
description: API reference for the WebsocketServer class.
---

The `WebsocketServer` class is the main entry point for creating a WebSocket server.

## Import

```python
from webrockets import WebsocketServer
```

For Django projects, use the pre-configured server singleton:

```python
from webrockets.django import server
```

## Constructor

```python
WebsocketServer(
    host: str = "0.0.0.0",
    port: int = 46290,
    broker: BrokerConfig | None = None
)
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `host` | `str` | `"0.0.0.0"` | Host address to bind to |
| `port` | `int` | `46290` | Port number to listen on |
| `broker` | `BrokerConfig \| None` | `None` | Message broker configuration for multi-server broadcasting |

### Broker Configuration

For Redis:

```python
broker = {
    "type": "redis",
    "url": "redis://localhost:6379",  # optional
    "channel": "ws_broadcast"          # optional
}
```

For RabbitMQ:

```python
broker = {
    "type": "amqp",
    "url": "amqp://localhost:5672",    # optional
    "exchange": "ws_broadcast",         # optional
    "queue": None,                      # optional, auto-generated if None
    "routing_key": "#"                  # optional
}
```

## Methods

### create_route

Create a new WebSocket route.

```python
create_route(
    path: str,
    default_group: str | None = None,
    authentication_classes: list[BaseAuthentication] | None = None
) -> WebsocketRoute
```

#### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `path` | `str` | - | URL path for the WebSocket endpoint |
| `default_group` | `str \| None` | `None` | Default group name for broadcasting. Connections are automatically added to this group. |
| `authentication_classes` | `list` | `None` | List of authentication class instances |

#### Returns

A `WebsocketRoute` instance for registering callbacks.

#### Example

```python
from webrockets import WebsocketServer
from webrockets.django.auth import SessionAuthentication

server = WebsocketServer(host="0.0.0.0", port=8080)

chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[SessionAuthentication()]
)
```

### start

Start the WebSocket server. This method blocks until the server is stopped.

```python
start() -> None
```

#### Example

```python
server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

@chat.receive
def on_message(conn, data):
    conn.send(data)

server.start()  # Blocks here
```

### stop

Stop the WebSocket server.

```python
stop() -> None
```

#### Example

```python
import signal

server = WebsocketServer()

def shutdown_handler(signum, frame):
    server.stop()

signal.signal(signal.SIGTERM, shutdown_handler)
server.start()
```

### addr

Get the address the server is bound to.

```python
addr() -> str
```

Returns the full address in `host:port` format.

#### Example

```python
server = WebsocketServer(host="127.0.0.1", port=8080)
print(server.addr())  # "127.0.0.1:8080"
```

## Django Integration

When using Django, import the pre-configured server singleton:

```python
from webrockets.django import server

# Server is already configured from Django settings:
# - WEBSOCKET_HOST (default: "0.0.0.0")
# - WEBSOCKET_PORT (default: 46290)
# - WEBSOCKET_BROKER (default: None)

chat = server.create_route("ws/chat/", "chat")
```

## Complete Example

```python
from webrockets import WebsocketServer

server = WebsocketServer(
    host="0.0.0.0",
    port=8080,
    broker={"type": "redis", "url": "redis://localhost:6379"}
)

echo = server.create_route("ws/echo/", "echo")

@echo.connect("before")
def on_connect(conn):
    print(f"Connected: {conn.path}")

@echo.receive
def on_message(conn, data):
    conn.send(data)

@echo.disconnect
def on_disconnect(conn, code=None, reason=None):
    print(f"Disconnected: {code}")

server.start()
```
