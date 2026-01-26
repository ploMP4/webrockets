---
title: Client
description: Built-in WebSocket client for testing and simple use cases.
---

:::caution[Early Development]
The WebSocket client is currently in early development and is mostly used for testing at the moment. It is not yet recommended for production use. For production applications, consider using established WebSocket client libraries like [websockets](https://websockets.readthedocs.io/).
:::

webrockets includes a built-in WebSocket client for testing your WebSocket applications. Both synchronous and asynchronous APIs are available.

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

## Limitations

The current client implementation has the following limitations:

- **No TLS/SSL support**: Only `ws://` URLs are supported, not `wss://`
- **No subprotocol negotiation**: Cannot specify WebSocket subprotocols
- **No timeouts**: Connection and read timeouts are not configurable
- **No max message size**: Cannot limit incoming message size
- **No compression**: `permessage-deflate` extension is not supported

## Next Steps

- [Testing Guide](/guides/testing/) - More testing patterns and examples
- [Reference: Client](/reference/client/) - Complete API reference
