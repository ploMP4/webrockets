---
title: Client
description: API reference for the WebSocket client classes and functions.
---

:::caution[Early Development]
The WebSocket client is in early development and is mostly used for testing at the moment. Not yet recommended for production use.
:::

## Import

```python
from webrockets.client import (
    # Functions
    connect,
    aconnect,
    # Classes
    Client,
    AsyncClient,
    ClientConfig,
    # Exceptions
    ConnectionClosed,
    InvalidStatusCode,
)
```

## Functions

### connect

Create a synchronous WebSocket connection.

```python
connect(url: str, config: ClientConfig | None = None) -> Client
```

#### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `url` | `str` | - | WebSocket URL to connect to (e.g., `ws://localhost:8080/ws/`) |
| `config` | `ClientConfig \| None` | `None` | Optional client configuration |

#### Returns

A connected `Client` instance.

#### Example

```python
from webrockets.client import connect

# Simple usage
with connect("ws://localhost:8080/ws/echo/") as ws:
    ws.send("Hello")
    print(ws.recv())

# Without context manager
ws = connect("ws://localhost:8080/ws/echo/")
ws.send("Hello")
print(ws.recv())
ws.close()
```

---

### aconnect

Create an asynchronous WebSocket connection for use with `async with`.

```python
aconnect(url: str, config: ClientConfig | None = None) -> AsyncClient
```

#### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `url` | `str` | - | WebSocket URL to connect to |
| `config` | `ClientConfig \| None` | `None` | Optional client configuration |

#### Returns

An `AsyncClient` instance (connects when entering async context).

#### Example

```python
import asyncio
from webrockets.client import aconnect

async def main():
    async with aconnect("ws://localhost:8080/ws/echo/") as ws:
        await ws.send("Hello")
        print(await ws.recv())

asyncio.run(main())
```

---

## Classes

### Client

Synchronous WebSocket client.

#### Constructor

```python
Client(config: ClientConfig | None = None)
```

#### Properties

| Property | Type | Description |
|----------|------|-------------|
| `config` | `ClientConfig` | The client configuration |

#### Methods

##### connect

Connect to a WebSocket server.

```python
connect(url: str) -> None
```

##### send

Send a message to the server.

```python
send(data: str | bytes) -> None
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `data` | `str \| bytes` | Message to send. Strings are sent as text frames, bytes as binary frames. |

##### recv

Receive a message from the server. Blocks until a message is received.

```python
recv() -> str | bytes
```

**Returns:** The received message. Text frames return `str`, binary frames return `bytes`.

**Raises:** `ConnectionClosed` if the server closes the connection.

##### close

Close the WebSocket connection.

```python
close(code: int = 1000, reason: str = "") -> None
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `code` | `int` | `1000` | WebSocket close code |
| `reason` | `str` | `""` | Close reason string |

#### Context Manager

`Client` supports the context manager protocol:

```python
with connect("ws://localhost:8080/ws/") as ws:
    ws.send("Hello")
    print(ws.recv())
# Connection is automatically closed
```

---

### AsyncClient

Asynchronous WebSocket client.

#### Constructor

```python
AsyncClient(config: ClientConfig | None = None)
```

#### Properties

| Property | Type | Description |
|----------|------|-------------|
| `config` | `ClientConfig` | The client configuration |

#### Methods

##### connect

Connect to a WebSocket server.

```python
async connect(url: str) -> None
```

##### send

Send a message to the server.

```python
async send(data: str | bytes) -> None
```

##### recv

Receive a message from the server.

```python
async recv() -> str | bytes
```

**Raises:** `ConnectionClosed` if the server closes the connection.

##### close

Close the WebSocket connection.

```python
async close(code: int = 1000, reason: str = "") -> None
```

#### Async Context Manager

`AsyncClient` supports the async context manager protocol:

```python
async with aconnect("ws://localhost:8080/ws/") as ws:
    await ws.send("Hello")
    print(await ws.recv())
# Connection is automatically closed
```

---

### ClientConfig

Configuration options for the WebSocket client.

#### Constructor

```python
ClientConfig(extra_headers: dict[str, str] | None = None)
```

#### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `extra_headers` | `dict[str, str] \| None` | `None` | Additional HTTP headers to send during the WebSocket handshake |

#### Example

```python
from webrockets.client import connect, ClientConfig

config = ClientConfig(extra_headers={
    "Authorization": "Bearer token123",
    "X-Custom-Header": "value",
})

with connect("ws://localhost:8080/ws/", config=config) as ws:
    ws.send("authenticated message")
```

---

## Exceptions

### ConnectionClosed

Raised when the WebSocket connection is closed by the server.

```python
class ConnectionClosed(Exception):
    code: int      # WebSocket close code
    reason: str    # Close reason (may be empty)
```

#### Example

```python
from webrockets.client import connect, ConnectionClosed

try:
    with connect("ws://localhost:8080/ws/") as ws:
        while True:
            msg = ws.recv()
            print(msg)
except ConnectionClosed as e:
    if e.code == 1000:
        print("Server closed normally")
    else:
        print(f"Server closed with code {e.code}: {e.reason}")
```

---

### InvalidStatusCode

Raised when the server returns a non-101 HTTP status code during the WebSocket handshake.

```python
class InvalidStatusCode(Exception):
    status_code: int  # HTTP status code
```

#### Example

```python
from webrockets.client import connect, InvalidStatusCode

try:
    with connect("ws://localhost:8080/ws/protected/") as ws:
        ws.send("Hello")
except InvalidStatusCode as e:
    if e.status_code == 401:
        print("Authentication required")
    elif e.status_code == 403:
        print("Access denied")
    elif e.status_code == 404:
        print("Endpoint not found")
    else:
        print(f"Server returned HTTP {e.status_code}")
```
