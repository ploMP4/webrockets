# pywsrs

[![PyPI](https://img.shields.io/pypi/v/pywsrs.svg)](https://pypi.org/project/pywsrs/)
[![Python](https://img.shields.io/pypi/pyversions/pywsrs.svg)](https://pypi.org/project/pywsrs/)
[![License](https://img.shields.io/pypi/l/pywsrs.svg)](https://github.com/kartofe/pywsrs/blob/main/LICENSE.md)

A high-performance WebSocket server for Python, with first-class Django support. The core server is implemented in Rust using PyO3 for maximum performance.

## Features

- **Blazing Fast** - Rust-powered WebSocket server using axum and fastwebsockets
- **Django Integration** - Built-in authentication classes and management commands
- **Pattern Matching** - Route messages based on discriminator fields with optional Pydantic validation
- **Broadcasting** - Built-in support for Redis and RabbitMQ message brokers
- **Async Ready** - Supports both sync and async Python callbacks

## Installation

```bash
# Basic installation
pip install pywsrs

# With Django integration
pip install pywsrs[django]

# With Pydantic schema validation
pip install pywsrs[schema]

# All extras
pip install pywsrs[schema,django]
```

## Quick Start

```python
from pywsrs import Websocket

# Create a WebSocket route
echo = Websocket("ws/echo/", "echo")

@echo.connect("before")
def on_connect(conn):
    print(f"Client connected: {conn.path}")

@echo.receive
def on_message(conn, data):
    # Echo the message back
    conn.send(f"You said: {data}")

@echo.disconnect
def on_disconnect(conn, code=None, reason=None):
    print(f"Client disconnected: {code}")

Websocket.start()
```

## Django Integration

pywsrs provides seamless Django integration with built-in authentication support.

### Setup

Add `pywsrs.django` to your `INSTALLED_APPS`:

```python
INSTALLED_APPS = [
    ...
    "pywsrs.django",
]
```

Create your WebSocket routes in a `websockets.py` file in any of your Django apps:

```python
# myapp/websockets.py
from pywsrs import Websocket
from pywsrs.django.auth import SessionAuthentication

chat = Websocket(
    "ws/chat/",
    "chat",
    authentication_classes=[SessionAuthentication()]
)

@chat.connect("before")
def on_connect(conn):
    print(f"User {conn.user} joined the chat")

@chat.receive
def on_message(conn, data):
    conn.send(f"{conn.user}: {data}")

@chat.disconnect
def on_disconnect(conn, code=None, reason=None):
    print(f"User {conn.user} left the chat")
```

Start the WebSocket server:

```bash
python manage.py runwebsockets
```

### Authentication Classes

pywsrs includes several authentication classes following Django REST Framework patterns:

```python
from pywsrs.django.auth import (
    SessionAuthentication,       # Django session-based auth
    CookieTokenAuthentication,   # Token from cookie
    HeaderTokenAuthentication,   # Token from header
    QueryStringTokenAuthentication,  # Token from URL query
)

# Use session auth (for browser clients)
chat = Websocket("ws/chat/", "chat", authentication_classes=[
    SessionAuthentication()
])

# Custom token authentication
class MyTokenAuth(CookieTokenAuthentication):
    cookie_name = "ws_token"

    def validate_token(self, token):
        # Return user object or None
        return User.objects.filter(auth_token=token).first()
```

## Pattern Matching

Route messages based on JSON fields using the `Match` class:

```python
from pydantic import BaseModel
from pywsrs import Match, Websocket

class ChatMessage(BaseModel):
    type: str
    content: str
    room: str

chat = Websocket("ws/chat/", "chat")

# Match on a single key/value with Pydantic validation
@chat.receive(match=Match("type", "message"), schema=ChatMessage)
def on_chat(conn, data: ChatMessage):
    conn.broadcast([data.room], data.content)

# Match without schema - data is the raw JSON string
@chat.receive(match=Match("type", "ping"))
def on_ping(conn, data: str):
    conn.send('{"type": "pong"}')

# Fallback for unmatched messages
@chat.receive
def on_fallback(conn, data):
    conn.send("Unknown message type")
```

### Multiple Keys and Values

Match on multiple keys (OR logic) or multiple values:

```python
# Match if "type" OR "action" field equals "message"
@chat.receive(match=Match(["type", "action"], "message"))
def on_message(conn, data):
    pass

# Match if "type" equals "msg" OR "message" OR "chat"
@chat.receive(match=Match("type", ["msg", "message", "chat"]))
def on_chat(conn, data):
    pass

# Combine both - match any key with any value
@chat.receive(match=Match(["type", "action"], ["ping", "health"]))
def on_health_check(conn, data):
    conn.send('{"status": "ok"}')
```

### Integer Values

Match on integer values in JSON:

```python
# Match {"code": 1}
@chat.receive(match=Match("code", 1))
def on_code_1(conn, data):
    pass

# Match multiple codes
@chat.receive(match=Match("code", [1, 2, 3]))
def on_codes(conn, data):
    pass

# Mix strings and integers
@chat.receive(match=Match("type", ["message", 1, 2]))
def on_mixed(conn, data):
    pass
```

### Wildcard Matching

Use `"*"` to match any value for a key:

```python
# Match any message that has a "type" field, regardless of value
@chat.receive(match=Match("type", "*"))
def on_any_typed_message(conn, data):
    pass

# Match if message has "type" OR "action" field with any value
@chat.receive(match=Match(["type", "action"], "*"))
def on_any_action(conn, data):
    pass
```

### Remove Matched Key

Strip the discriminator key from the JSON before passing to the handler:

```python
# Input: {"type": "chat", "content": "hello"}
# Handler receives: {"content": "hello"}
@chat.receive(match=Match("type", "chat", remove_key=True))
def on_chat(conn, data):
    pass
```

## Broadcasting

Broadcast messages to all clients in a group:

```python
from pywsrs import Websocket

chat = Websocket("ws/chat/", "chat")

@chat.receive
def on_message(conn, data):
    # Broadcast to all clients in the "chat" group
    conn.broadcast(["chat"], data)

    # Or send only to specific groups
    conn.broadcast(["room_1", "room_2"], data)
```

### Multi-Server Broadcasting

For multi-server deployments, configure a message broker:

```python
from pywsrs import setup_broadcast, broadcast

# Redis
setup_broadcast({"type": "redis", "url": "redis://localhost:6379"})

# RabbitMQ
setup_broadcast({"type": "amqp", "url": "amqp://guest:guest@localhost:5672"})

# Broadcast across all server instances
broadcast(["chat"], "Hello from any server!")
```

## Server Close

Close connections from the server side:

```python
@chat.receive
def on_message(conn, data):
    if data == "goodbye":
        conn.close()  # Close with default code 1000
    elif data == "kick":
        conn.close(code=1008, reason="Policy violation")
```

## Async Callbacks

Use async functions for callbacks:

```python
@chat.connect("before")
async def on_connect(conn):
    await some_async_operation()
    print(f"User connected: {conn.user}")

@chat.receive
async def on_message(conn, data):
    result = await fetch_from_database(data)
    await conn.asend(result)
```


- [ ] remove weird start method
- [ ] logging examples
- [ ] add on request log
- [ ] turn off logging option
