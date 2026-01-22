---
title: Connection
description: API reference for the Connection class.
---

The `Connection` class represents an active WebSocket connection. It provides access to request information and methods for sending messages.

## Properties

### user

The authenticated user, set by authentication classes.

```python
user: Any | None
```

```python
@chat.receive
def on_message(conn, data):
    if conn.user:
        print(f"Message from {conn.user.username}")
```

### path

The URL path of the WebSocket connection.

```python
path: str
```

```python
@chat.connect("before")
def on_connect(conn):
    print(f"Path: {conn.path}")  # "/ws/chat/"
```

### query_string

The query string from the connection URL (without the leading `?`).

```python
query_string: str
```

```python
# URL: ws://example.com/ws/chat/?room=general&token=abc
@chat.connect("before")
def on_connect(conn):
    print(conn.query_string)  # "room=general&token=abc"
```

To parse query parameters:

```python
from urllib.parse import parse_qs

@chat.connect("before")
def on_connect(conn):
    params = parse_qs(conn.query_string)
    room = params.get("room", ["default"])[0]
```

### headers

Dictionary containing all HTTP headers from the connection request.

```python
headers: dict[str, str]
```

```python
@chat.connect("before")
def on_connect(conn):
    # Access headers directly
    for name, value in conn.headers.items():
        print(f"{name}: {value}")
```

### cookies

Dictionary containing all cookies from the connection request.

```python
cookies: dict[str, str]
```

```python
@chat.connect("before")
def on_connect(conn):
    # Access cookies directly
    for name, value in conn.cookies.items():
        print(f"{name}: {value}")
```

## Methods

### send

Send a message to this connection (synchronous).

```python
send(message: str | bytes) -> None
```

```python
@chat.receive
def on_message(conn, data):
    conn.send("Hello!")
    conn.send('{"status": "ok"}')
    conn.send(b"\x00\x01\x02")  # Binary message
```

### asend

Send a message to this connection (asynchronous).

```python
async asend(message: str | bytes) -> None
```

```python
@chat.receive
async def on_message(conn, data):
    await conn.asend("Hello!")
    await conn.asend(b"\x00\x01\x02")  # Binary message
```

### broadcast

Send a message to all connections in the specified groups (synchronous).

```python
broadcast(groups: list[str], message: str | bytes, exclude_self: bool = False) -> None
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `groups` | `list[str]` | - | List of group names to broadcast to |
| `message` | `str \| bytes` | - | Message to send |
| `exclude_self` | `bool` | `False` | If True, exclude this connection from receiving the broadcast |

```python
@chat.receive
def on_message(conn, data):
    # Broadcast to all clients in the "chat" group
    conn.broadcast(["chat"], data)

    # Broadcast to multiple groups
    conn.broadcast(["chat", "notifications"], "Hello everyone!")

    # Broadcast but don't echo back to sender
    conn.broadcast(["chat"], data, exclude_self=True)
```

The group name is typically the second argument passed to `server.create_route()` (the `default_group`).

### abroadcast

Send a message to all connections in the specified groups (asynchronous).

```python
async abroadcast(groups: list[str], message: str | bytes, exclude_self: bool = False) -> None
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `groups` | `list[str]` | - | List of group names to broadcast to |
| `message` | `str \| bytes` | - | Message to send |
| `exclude_self` | `bool` | `False` | If True, exclude this connection from receiving the broadcast |

```python
@chat.receive
async def on_message(conn, data):
    await conn.abroadcast(["chat"], data)
    await conn.abroadcast(["chat"], data, exclude_self=True)
```

### close

Close the WebSocket connection.

```python
close(code: int = 1000, reason: str = "") -> None
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `code` | `int` | `1000` | WebSocket close code |
| `reason` | `str` | `""` | Close reason message |

```python
@chat.receive
def on_message(conn, data):
    if data == "goodbye":
        conn.close()  # Normal close

    if data == "kick":
        conn.close(code=1008, reason="Policy violation")
```

### aclose

Close the WebSocket connection (asynchronous).

```python
async aclose(code: int = 1000, reason: str = "") -> None
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `code` | `int` | `1000` | WebSocket close code |
| `reason` | `str` | `""` | Close reason message |

```python
@chat.receive
async def on_message(conn, data):
    if data == "goodbye":
        await conn.aclose()
```

### join

Join a group dynamically. Returns `True` if newly joined, `False` if already a member.

```python
join(group: str) -> bool
```

```python
@chat.receive(match=Match("type", "join_room"))
def on_join_room(conn, data):
    room = data.get("room", "general")
    if conn.join(room):
        conn.broadcast([room], f"User joined {room}")
```

### leave

Leave a group dynamically. Returns `True` if was a member, `False` otherwise.

```python
leave(group: str) -> bool
```

```python
@chat.receive(match=Match("type", "leave_room"))
def on_leave_room(conn, data):
    room = data.get("room")
    if conn.leave(room):
        conn.broadcast([room], f"User left {room}")
```

### groups

Get all groups this connection currently belongs to.

```python
groups() -> list[str]
```

```python
@chat.receive(match=Match("type", "list_rooms"))
def on_list_rooms(conn, data):
    current_groups = conn.groups()
    conn.send(f'{{"rooms": {current_groups}}}')
```

### group_size

Get the number of connections in a group.

```python
group_size(group: str) -> int
```

```python
@chat.receive(match=Match("type", "room_info"))
def on_room_info(conn, data):
    room = data.get("room")
    count = conn.group_size(room)
    conn.send(f'{{"room": "{room}", "users": {count}}}')
```

### get_cookie

Get a cookie value from the connection request.

```python
get_cookie(name: str) -> str | None
```

```python
@chat.connect("before")
def on_connect(conn):
    session_id = conn.get_cookie("sessionid")
    jwt_token = conn.get_cookie("jwt")
```

### get_header

Get a header value from the connection request.

```python
get_header(name: str) -> str | None
```

Header names are case-insensitive.

```python
@chat.connect("before")
def on_connect(conn):
    origin = conn.get_header("origin")
    auth = conn.get_header("authorization")
    user_agent = conn.get_header("user-agent")
```

## IncomingConnection

During the `connect("before")` phase, the connection object is an `IncomingConnection` which has the same properties and `get_cookie`/`get_header` methods, but not `send`, `broadcast`, or `close` since the connection is not yet established.

```python
@chat.connect("before")
def before_connect(conn):  # conn is IncomingConnection
    # Can access request info
    print(conn.path)
    print(conn.get_cookie("sessionid"))

    # Cannot send yet
    # conn.send("hi")  # Would raise error

@chat.connect("after")
def after_connect(conn):  # conn is Connection
    # Now can send
    conn.send("Welcome!")
```

## Complete Example

```python
from urllib.parse import parse_qs
from webrockets import WebsocketServer

server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

@chat.connect("before")
def validate(conn):
    # Access request info
    print(f"Path: {conn.path}")
    print(f"Origin: {conn.get_header('origin')}")

    # Parse query string
    params = parse_qs(conn.query_string)
    room = params.get("room", ["general"])[0]
    print(f"Room: {room}")

@chat.connect("after")
def welcome(conn):
    conn.send('{"event": "connected"}')

@chat.receive
def on_message(conn, data):
    # Echo back
    conn.send(f"You said: {data}")

    # Broadcast to all clients on the route
    conn.broadcast(["chat"], data)

@chat.disconnect
def on_disconnect(conn, code=None, reason=None):
    print(f"Disconnected: code={code}")

server.start()
```
