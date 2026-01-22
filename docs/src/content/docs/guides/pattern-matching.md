---
title: Pattern Matching
description: Route WebSocket messages based on JSON fields.
---

webrockets can route incoming messages to different handlers based on JSON field values. This is useful for implementing message protocols where each message has a type or action field.

## Basic Usage

Use the `Match` class to route messages:

```python
from webrockets import Match, WebsocketServer

server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

@chat.receive(match=Match("type", "message"))
def on_message(conn, data):
    # Only called when {"type": "message", ...}
    conn.send("Got a message!")

@chat.receive(match=Match("type", "ping"))
def on_ping(conn, data):
    # Only called when {"type": "ping", ...}
    conn.send('{"type": "pong"}')

@chat.receive
def on_fallback(conn, data):
    # Called for unmatched messages
    conn.send("Unknown message type")
```

## How It Works

1. Incoming JSON is parsed once in Rust using `serde_json`
2. The discriminator field is checked against registered handlers
3. The first matching handler receives the message
4. If no handler matches, the fallback handler is called

## Match Patterns

### Single Key and Value

Match a specific field with a specific value:

```python
@chat.receive(match=Match("type", "chat"))
def on_chat(conn, data):
    pass
```

Matches: `{"type": "chat", ...}`

### Multiple Values

Match any of several values:

```python
@chat.receive(match=Match("type", ["join", "leave"]))
def on_room_event(conn, data):
    pass
```

Matches: `{"type": "join", ...}` or `{"type": "leave", ...}`

### Multiple Keys (OR Logic)

Check multiple keys for the value:

```python
@chat.receive(match=Match(["type", "action"], "ping"))
def on_ping(conn, data):
    pass
```

Matches: `{"type": "ping", ...}` or `{"action": "ping", ...}`

### Combined Keys and Values

Match any key with any value:

```python
@chat.receive(match=Match(["type", "action"], ["ping", "pong"]))
def on_ping_pong(conn, data):
    pass
```

Matches: `{"type": "ping"}`, `{"type": "pong"}`, `{"action": "ping"}`, or `{"action": "pong"}`

### Integer Values

Match numeric values:

```python
@chat.receive(match=Match("code", 1))
def on_code_1(conn, data):
    pass

@chat.receive(match=Match("code", [1, 2, 3]))
def on_codes(conn, data):
    pass
```

Matches: `{"code": 1}`, `{"code": 2}`, or `{"code": 3}`

### Mixed Types

Combine strings and integers:

```python
@chat.receive(match=Match("type", ["message", 1, 2]))
def on_mixed(conn, data):
    pass
```

### Wildcard Matching

Match any value for a key using `"*"`:

```python
@chat.receive(match=Match("type", "*"))
def on_any_type(conn, data):
    # Called for any message with a "type" field
    pass
```

Matches any message containing the "type" field, regardless of its value.

## Remove Matched Key

Strip the discriminator from the message before passing to the handler:

```python
# Input: {"type": "chat", "content": "hello", "room": "general"}
# Handler receives: {"content": "hello", "room": "general"}

@chat.receive(match=Match("type", "chat", remove_key=True))
def on_chat(conn, data):
    # data does not contain "type"
    pass
```

## Pydantic Validation

Validate messages with Pydantic schemas. Requires the `schema` extra:

```bash
pip install webrockets[schema]
```

```python
from typing import Literal
from pydantic import BaseModel
from webrockets import Match, WebsocketServer

class ChatMessage(BaseModel):
    type: Literal["chat"]
    content: str
    room: str

server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

@chat.receive(match=Match("type", "chat"), schema=ChatMessage)
def on_chat(conn, data: ChatMessage):
    # data is a validated ChatMessage instance
    conn.broadcast([data.room], data.content)

@chat.receive
def on_invalid(conn, data):
    # Called if validation fails or no match
    conn.send("Invalid message")
```

## Handler Priority

Handlers are matched in registration order. More specific handlers should be registered first:

```python
# Register specific handlers first
@chat.receive(match=Match("type", "special"))
def on_special(conn, data):
    pass

# Then more general handlers
@chat.receive(match=Match("type", "*"))
def on_any(conn, data):
    pass

# Fallback last
@chat.receive
def on_fallback(conn, data):
    pass
```

## Complete Example

```python
from typing import Literal
from pydantic import BaseModel
from webrockets import Match, WebsocketServer

class JoinMessage(BaseModel):
    type: Literal["join"]
    room: str

class ChatMessage(BaseModel):
    type: Literal["chat"]
    room: str
    content: str

server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

@chat.receive(match=Match("type", "join"), schema=JoinMessage)
def on_join(conn, data: JoinMessage):
    conn.send(f"Joined {data.room}")

@chat.receive(match=Match("type", "chat"), schema=ChatMessage)
def on_chat(conn, data: ChatMessage):
    conn.broadcast([data.room], f"{data.content}")

@chat.receive(match=Match("type", "ping"))
def on_ping(conn, data):
    conn.send('{"type": "pong"}')

@chat.receive
def on_unknown(conn, data):
    conn.send('{"error": "Unknown message type"}')

if __name__ == "__main__":
    server.start()
```
