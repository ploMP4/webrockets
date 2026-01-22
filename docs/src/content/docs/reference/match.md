---
title: Match
description: API reference for the Match class.
---

The `Match` class defines patterns for routing WebSocket messages based on JSON field values.

## Import

```python
from webrockets import Match
```

## Constructor

```python
Match(
    keys: str | list[str],
    values: str | int | list[str | int],
    remove_key: bool = False
)
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `keys` | `str \| list[str]` | - | JSON key(s) to match against |
| `values` | `str \| int \| list[str \| int]` | - | Value(s) to match |
| `remove_key` | `bool` | `False` | Remove matched key from message |

## Patterns

### Single Key, Single Value

Match messages where a specific key has a specific value:

```python
Match("type", "message")
```

Matches: `{"type": "message", ...}`

### Single Key, Multiple Values

Match messages where a key has any of the specified values:

```python
Match("type", ["join", "leave", "message"])
```

Matches:
- `{"type": "join", ...}`
- `{"type": "leave", ...}`
- `{"type": "message", ...}`

### Multiple Keys, Single Value

Match messages where any of the specified keys has the value (OR logic):

```python
Match(["type", "action"], "ping")
```

Matches:
- `{"type": "ping", ...}`
- `{"action": "ping", ...}`

### Multiple Keys, Multiple Values

Match messages where any key has any value:

```python
Match(["type", "action"], ["ping", "pong"])
```

Matches:
- `{"type": "ping", ...}`
- `{"type": "pong", ...}`
- `{"action": "ping", ...}`
- `{"action": "pong", ...}`

### Integer Values

Match numeric values in JSON:

```python
Match("code", 1)
Match("code", [1, 2, 3])
Match("code", [1, "one", 2, "two"])  # Mixed types
```

Matches: `{"code": 1}`, `{"code": 2}`, etc.

### Wildcard

Match any value for a key using `"*"`:

```python
Match("type", "*")
```

Matches any message with a "type" field, regardless of value:
- `{"type": "anything", ...}`
- `{"type": 123, ...}`
- `{"type": null, ...}`

### Remove Key

Strip the matched key from the message:

```python
Match("type", "message", remove_key=True)
```

Input: `{"type": "message", "content": "hello"}`
Handler receives: `{"content": "hello"}`

## Usage with receive

```python
from webrockets import Match, WebsocketServer

server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

# Basic match
@chat.receive(match=Match("type", "ping"))
def on_ping(conn, data):
    conn.send('{"type": "pong"}')

# Multiple values
@chat.receive(match=Match("type", ["join", "leave"]))
def on_room_event(conn, data):
    pass

# Wildcard
@chat.receive(match=Match("type", "*"))
def on_any_type(conn, data):
    pass

# Fallback for non-JSON or unmatched
@chat.receive
def on_fallback(conn, data):
    pass
```

## Usage with Pydantic

Combine pattern matching with schema validation:

```python
from pydantic import BaseModel
from webrockets import Match, WebsocketServer

class ChatMessage(BaseModel):
    type: str
    content: str
    room: str

server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

@chat.receive(match=Match("type", "chat"), schema=ChatMessage)
def on_chat(conn, data: ChatMessage):
    # data is validated ChatMessage
    conn.broadcast([data.room], data.content)
```

Validation flow:
1. Pattern match is checked first
2. If matched, Pydantic validates the full message
3. If validation fails, fallback handler is called

## Handler Priority

Handlers are matched in registration order. Register specific handlers before general ones:

```python
# Specific - checked first
@chat.receive(match=Match("type", "special"))
def on_special(conn, data):
    pass

# General wildcard - checked second
@chat.receive(match=Match("type", "*"))
def on_any_type(conn, data):
    pass

# Fallback - checked last
@chat.receive
def on_fallback(conn, data):
    pass
```

## Performance

Pattern matching is optimized in Rust:
- JSON is parsed once using `serde_json`
- Handlers are compiled into a lock-free data structure
- Matching is O(1) for single-value patterns
- No Python calls during the matching phase

## Examples

### Chat Protocol

```python
from webrockets import Match

# System messages
@chat.receive(match=Match("type", "ping"))
def on_ping(conn, data):
    conn.send('{"type": "pong"}')

# Room events
@chat.receive(match=Match("type", ["join", "leave"]))
def on_room_event(conn, data):
    pass

# Chat messages with validation
@chat.receive(match=Match("type", "message"), schema=ChatMessage)
def on_message(conn, data: ChatMessage):
    conn.broadcast([data.room], data.content)

# Unknown types
@chat.receive
def on_unknown(conn, data):
    conn.send('{"error": "Unknown message type"}')
```

### API Actions

```python
from webrockets import Match

# Match by action field
@api.receive(match=Match("action", "subscribe"))
def on_subscribe(conn, data):
    pass

@api.receive(match=Match("action", "unsubscribe"))
def on_unsubscribe(conn, data):
    pass

# Match by numeric code
@api.receive(match=Match("code", [100, 101, 102]))
def on_info_codes(conn, data):
    pass
```
