---
title: Client
description: Built-in WebSocket client for testing and simple use cases.
---

webrockets includes a built-in WebSocket client. Both synchronous and asynchronous APIs are available, with support for TLS, timeouts, subprotocol negotiation, and ping/pong.

## Import

```python
# Synchronous client
from webrockets.client import connect, Client, ClientConfig

# Asynchronous client
from webrockets.client import aconnect, AsyncClient
```

## Synchronous Client

### Basic Usage

The simplest way to use the client is with the `connect()` function and a context manager:

```python
from webrockets.client import connect

with connect("ws://localhost:8080/ws/echo/") as ws:
    ws.send("Hello, server!")
    response = ws.recv()
    print(response)
```

### Manual Connection

You can also create a `Client` instance and connect manually:

```python
from webrockets.client import Client

client = Client()
client.connect("ws://localhost:8080/ws/echo/")

client.send("Hello!")
response = client.recv()
print(response)

client.close()
```

### Custom Headers

Use `ClientConfig` to add custom headers for authentication:

```python
from webrockets.client import connect, ClientConfig

config = ClientConfig(extra_headers={
    "Authorization": "Bearer your-token-here",
    "Cookie": "sessionid=abc123",
})

with connect("ws://localhost:8080/ws/chat/", config=config) as ws:
    ws.send("Hello!")
    print(ws.recv())
```

### TLS / Secure Connections

Use `wss://` URLs and set `verify_ssl=True` to connect securely. The client uses the platform certificate store:

```python
from webrockets.client import connect, ClientConfig

config = ClientConfig(verify_ssl=True)

with connect("wss://api.example.com/ws/", config=config) as ws:
    ws.send("Hello!")
    print(ws.recv())
```

Set `verify_ssl=False` to skip certificate verification (useful during development with self-signed certificates):

```python
config = ClientConfig(verify_ssl=False)
ws = connect("wss://localhost:8443/ws/", config=config)
```

### Timeouts

Pass a timeout (in milliseconds) to `connect()` or `recv()`:

```python
from webrockets.client import connect

# Timeout on the initial connection
ws = connect("ws://localhost:8080/ws/echo/", timeout=5000)  # 5 seconds

# Timeout on a single recv call
try:
    msg = ws.recv(timeout=10000)  # 10 seconds
except TimeoutError:
    print("No message received in time")
```

### Ping / Pong

Send heartbeat frames to keep a connection alive:

```python
with connect("ws://localhost:8080/ws/echo/") as ws:
    ws.ping()              # empty ping
    ws.ping("heartbeat")   # ping with a payload
    ws.pong()              # manual pong
```

### Subprotocol Negotiation

Request subprotocols via `ClientConfig`. After connecting, `negotiated_protocol` contains the one the server selected (or `None`):

```python
from webrockets.client import connect, ClientConfig

config = ClientConfig(subprotocols=["graphql-ws", "graphql-transport-ws"])

with connect("wss://api.example.com/graphql", config=config) as ws:
    print(ws.negotiated_protocol)  # e.g. "graphql-ws"
    ws.send('{"type": "connection_init"}')
```

### Max Message Size

Limit the maximum incoming message size (in bytes) to prevent memory issues:

```python
config = ClientConfig(max_message_size=16 * 1024 * 1024)  # 16 MB
```

## Asynchronous Client

### Basic Usage

For async applications, use `aconnect()` with an async context manager:

```python
import asyncio
from webrockets.client import aconnect

async def main():
    async with aconnect("ws://localhost:8080/ws/echo/") as ws:
        await ws.send("Hello, server!")
        response = await ws.recv()
        print(response)

asyncio.run(main())
```

The async client accepts the same `ClientConfig` options and `timeout` parameter:

```python
from webrockets.client import aconnect, ClientConfig

config = ClientConfig(
    verify_ssl=True,
    subprotocols=["chat"],
)

async with aconnect("wss://api.example.com/ws/", config=config, timeout=5000) as ws:
    print(ws.negotiated_protocol)
    await ws.send("Hello!")
    msg = await ws.recv(timeout=10000)
    await ws.ping()
```

## Exception Handling

The client raises specific exceptions for connection issues:

```python
from webrockets.client import connect, ConnectionClosed, InvalidStatusCode

try:
    with connect("ws://localhost:8080/ws/echo/") as ws:
        ws.send("Hello!")
        while True:
            msg = ws.recv()
            print(msg)
except ConnectionClosed as e:
    print(f"Connection closed: code={e.code}, reason={e.reason}")
except InvalidStatusCode as e:
    print(f"Server returned HTTP {e.status_code}")
```

### ConnectionClosed

Raised when the server closes the connection. Contains:
- `code`: The WebSocket close code (e.g., 1000 for normal closure)
- `reason`: Optional close reason string

### InvalidStatusCode

Raised when the server returns a non-101 HTTP status during the WebSocket handshake. Contains:
- `status_code`: The HTTP status code returned

## Testing Example

The client is designed to work seamlessly with the `runserver` context manager for testing:

```python
from webrockets.client import connect
from webrockets.test import runserver
from myapp.websockets import server

def test_echo():
    with runserver(server):
        with connect(f"ws://{server.addr()}/ws/echo/") as ws:
            ws.send("test message")
            assert ws.recv() == "echo: test message"
```

See the [Testing Guide](/guides/testing/) for more testing patterns.

## Next Steps

- [Testing Guide](/guides/testing/) - More testing patterns and examples
- [Reference: Client](/reference/client/) - Complete API reference
