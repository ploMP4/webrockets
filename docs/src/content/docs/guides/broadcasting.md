---
title: Broadcasting
description: Send messages to connected clients.
---

webrockets provides two ways to broadcast messages to clients:

- **`conn.broadcast()`** - Sends to clients connected to the **same server process**
- **`broadcast()`** function - Sends via a broker to clients on **all server processes**

## `conn.broadcast()` - Same Server

Use `conn.broadcast()` within WebSocket handlers to send messages to clients connected to the same server instance:

```python
from webrockets import WebsocketServer

server = WebsocketServer()
chat = server.create_route("ws/chat/", "chat")

@chat.receive
def on_message(conn, data):
    # Sends to clients on THIS server only
    conn.broadcast(["chat"], data)
```

The first argument is a list of group names. Clients must be subscribed to receive messages:

```python
@chat.connect("before")
def on_connect(conn):
    conn.join(["chat", "announcements"])
```

:::note
`conn.broadcast()` does NOT use a message broker. It only reaches clients in the same process. For single-server standalone deployments, this is sufficient.
:::

## `broadcast()` - All Servers

Use the `broadcast()` function when you need to reach clients across multiple server processes. This requires a message broker.

```python
from webrockets import broadcast

# Sends via the broker to ALL server processes
broadcast(["chat"], '{"type": "message", "content": "Hello everyone!"}')
```

### When to Use `broadcast()`

- Running multiple webrockets instances
- Sending messages from outside WebSocket handlers
- Sending messages from a separate process (Django, Celery, etc.)

## Configuring a Broker

### Standalone Python

```python
from webrockets import WebsocketServer

server = WebsocketServer(
    host="0.0.0.0",
    port=8080,
    broker={"type": "redis", "url": "redis://localhost:6379"}
)
```

### Django

```python
# settings.py
WEBSOCKET_BROKER = {
    "type": "redis",
    "url": "redis://localhost:6379",
}
```

:::tip[Django Users]
If you're using Django, see the [Django Broadcasting guide](/django/broadcasting/) for complete documentation on sending messages from Django views, signals, and Celery tasks.
:::

## Broker Options

### Redis

```python
broker = {
    "type": "redis",
    "url": "redis://localhost:6379",  # Connection URL
    "channel": "ws_broadcast",         # Pub/sub channel (optional)
}
```

| Option | Default | Description |
|--------|---------|-------------|
| `type` | - | Must be `"redis"` |
| `url` | `"redis://localhost:6379"` | Redis connection URL |
| `channel` | `"ws_broadcast"` | Redis pub/sub channel name |

### RabbitMQ

```python
broker = {
    "type": "amqp",
    "url": "amqp://guest:guest@localhost:5672",
    "exchange": "ws_broadcast",  # Exchange name (optional)
}
```

| Option | Default | Description |
|--------|---------|-------------|
| `type` | - | Must be `"amqp"` |
| `url` | `"amqp://localhost:5672"` | AMQP connection URL |
| `exchange` | `"ws_broadcast"` | Exchange name |
| `queue` | Auto-generated UUID | Queue name for this server |
| `routing_key` | `"#"` | Routing key pattern |

## Broadcasting from Anywhere

For standalone Python applications, use `setup_broadcast()` to configure the broker, then use `broadcast()` from anywhere:

```python
from webrockets import setup_broadcast, broadcast

# Configure once at startup
setup_broadcast({"type": "redis", "url": "redis://localhost:6379"})

# Broadcast from anywhere in your application
def send_notification(message):
    broadcast(["notifications"], message)
```

## Architecture

How broadcasting works with a broker:

```
┌─────────────────┐     ┌─────────────────┐
│  Server 1       │     │  Server 2       │
│  Clients A, B   │     │  Clients C, D   │
└────────┬────────┘     └────────┬────────┘
         │                       │
         └───────────┬───────────┘
                     │
              ┌──────▼──────┐
              │   Redis /   │
              │  RabbitMQ   │
              └─────────────┘
```

When using `broadcast()`:
1. Message is published to the broker
2. All server instances receive the message
3. Each server sends to its local subscribed clients

When using `conn.broadcast()`:
1. Message is sent only to clients on the current server
2. No broker involved
3. Clients on other servers do NOT receive the message

## Summary

| Method | Scope | Broker Required | Use Case |
|--------|-------|-----------------|----------|
| `conn.broadcast()` | Same server | No | Single-server deployments, within handlers |
| `broadcast()` | All servers | Yes | Multi-server, external processes, Django |

## Example

A complete example with both methods:

```python
from webrockets import WebsocketServer, broadcast

server = WebsocketServer(
    host="0.0.0.0",
    port=8080,
    broker={"type": "redis", "url": "redis://localhost:6379"}
)

chat = server.create_route("ws/chat/", "chat")

@chat.connect("before")
def on_connect(conn):
    conn.join(["chat"])

@chat.receive
def on_message(conn, data):
    # Option 1: Same server only (faster, no broker)
    # conn.broadcast(["chat"], data)

    # Option 2: All servers (uses broker)
    broadcast(["chat"], data)

server.start()
```
