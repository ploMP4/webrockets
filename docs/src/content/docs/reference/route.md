---
title: WebsocketRoute
description: API reference for the WebsocketRoute class.
---

The `WebsocketRoute` class represents a WebSocket endpoint and provides decorators for registering event handlers.

## Import

`WebsocketRoute` instances are created by calling `server.create_route()`:

```python
from webrockets import WebsocketServer

server = WebsocketServer()
route = server.create_route("ws/chat/", "chat")
```

## Properties

### path

The URL path for this WebSocket route.

```python
path: str
```

### default_group

The default group name for this route. Connections are automatically added to this group when they connect.

```python
default_group: str | None
```

### authentication_classes

List of authentication class instances configured for this route.

```python
authentication_classes: list[BaseAuthentication] | None
```

## Decorators

### connect

Register a callback for connection events.

```python
@route.connect(when: str)
def callback(conn: Connection) -> None:
    pass
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `when` | `str` | Either `"before"` or `"after"` |

#### Phases

- `"before"` - Called during the WebSocket handshake, before the connection is accepted. Use this for validation or early rejection.
- `"after"` - Called after the connection is established. The connection is active and can send messages.

#### Example

```python
@chat.connect("before")
def before_connect(conn):
    # Validate the connection
    if not conn.user:
        raise Exception("Not authenticated")

@chat.connect("after")
def after_connect(conn):
    # Connection is active
    conn.send("Welcome!")
```

### receive

Register a callback for incoming messages.

```python
@route.receive
def callback(conn: Connection, data: str) -> None:
    pass

# With pattern matching
@route.receive(match: Match, schema: type[BaseModel] | None = None)
def callback(conn: Connection, data: str | BaseModel) -> None:
    pass
```

#### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `match` | `Match` | `None` | Pattern to match against incoming JSON messages |
| `schema` | `type[BaseModel]` | `None` | Pydantic model for validation |

#### Example

```python
from webrockets import Match
from pydantic import BaseModel

class ChatMessage(BaseModel):
    type: str
    content: str

# Simple handler
@chat.receive
def on_message(conn, data):
    conn.send(f"Got: {data}")

# Pattern matching
@chat.receive(match=Match("type", "ping"))
def on_ping(conn, data):
    conn.send('{"type": "pong"}')

# With Pydantic validation
@chat.receive(match=Match("type", "chat"), schema=ChatMessage)
def on_chat(conn, data: ChatMessage):
    conn.broadcast(["chat"], data.content)
```

### disconnect

Register a callback for disconnection events.

```python
@route.disconnect
def callback(conn: Connection, code: int | None = None, reason: str | None = None) -> None:
    pass
```

#### Parameters

The callback receives:

| Parameter | Type | Description |
|-----------|------|-------------|
| `conn` | `Connection` | The connection object |
| `code` | `int \| None` | WebSocket close code |
| `reason` | `str \| None` | Close reason message |

#### Example

```python
@chat.disconnect
def on_disconnect(conn, code=None, reason=None):
    print(f"Client disconnected: code={code}, reason={reason}")
```

## Async Callbacks

All callbacks support async functions:

```python
@chat.connect("before")
async def on_connect(conn):
    await validate_user(conn.user)

@chat.receive
async def on_message(conn, data):
    result = await process_message(data)
    await conn.asend(result)

@chat.disconnect
async def on_disconnect(conn, code=None, reason=None):
    await cleanup_user(conn.user)
```

## Multiple Handlers

For `receive`, handlers are matched in registration order:

```python
# Specific handlers first
@chat.receive(match=Match("type", "special"))
def on_special(conn, data):
    pass

# Fallback last
@chat.receive
def on_any(conn, data):
    pass
```

## Complete Example

```python
from webrockets import Match, WebsocketServer
from pydantic import BaseModel

class Message(BaseModel):
    type: str
    content: str

server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

@chat.connect("before")
def validate(conn):
    if "blocked" in conn.query_string:
        raise Exception("Blocked")

@chat.connect("after")
def welcome(conn):
    conn.send('{"type": "connected"}')

@chat.receive(match=Match("type", "message"), schema=Message)
def on_message(conn, data: Message):
    conn.broadcast(["chat"], data.content)

@chat.receive(match=Match("type", "ping"))
def on_ping(conn, data):
    conn.send('{"type": "pong"}')

@chat.receive
def on_unknown(conn, data):
    conn.send('{"error": "Unknown message type"}')

@chat.disconnect
def on_disconnect(conn, code=None, reason=None):
    print(f"Goodbye: {code}")

server.start()
```
