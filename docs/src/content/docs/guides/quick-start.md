---
title: Quick Start
description: Get up and running with webrockets in 5 minutes.
---

This guide shows how to create a basic WebSocket server with webrockets.

## Installation

```bash
pip install webrockets
```

## Basic Server

Create a file called `server.py`:

```python
from webrockets import WebsocketServer, IncomingConnection, Connection

# Create server instance
server = WebsocketServer(host="0.0.0.0", port=8080)

# Create a WebSocket route
echo = server.create_route("ws/echo/", "echo")

@echo.connect("before")
def on_connect(conn: IncomingConnection):
    print(f"Client connected: {conn.path}")

@echo.receive
def on_message(conn: Connection, data: str | bytes):
    # Echo the message back to the client
    conn.send(f"You said: {data}")

@echo.disconnect
def on_disconnect(conn: Connection, code: int | None = None, reason: str | None = None):
    print(f"Client disconnected with code: {code}")

if __name__ == "__main__":
    # Start the server (blocks)
    server.start()
```

Run the server:

```bash
python server.py
```

## Testing the Connection

Connect using a WebSocket client. Here's a simple browser test:

```javascript
const ws = new WebSocket('ws://localhost:8080/ws/echo/');

ws.onopen = () => {
    console.log('Connected');
    ws.send('Hello, server!');
};

ws.onmessage = (event) => {
    console.log('Received:', event.data);
};

ws.onclose = (event) => {
    console.log('Disconnected:', event.code);
};
```

## Route Callbacks

Each route supports three callback types:

### connect

Called when a client connects. Use the `"before"` or `"after"` phase:

```python
@route.connect("before")  # Before accepting the connection
def before_connect(conn: IncomingConnection):
    # Access request info via conn
    print(conn.path, conn.query_string)

@route.connect("after")   # After connection is established
def after_connect(conn: Connection):
    # Connection is now active, can send messages
    conn.send("Welcome!")
```

### receive

Called when a message is received:

```python
@route.receive
def on_message(conn: Connection, data: str | bytes):
    # data is the message payload (string)
    conn.send(f"Got: {data}")
```

### disconnect

Called when a client disconnects:

```python
@route.disconnect
def on_disconnect(conn: Connection, code: int | None = None, reason: str | None = None):
    # code is the WebSocket close code
    # reason is the close reason string (if provided)
    print(f"Goodbye! Code: {code}")
```

## Async Callbacks

All callbacks support async functions:

```python
@echo.connect("before")
async def on_connect(conn: Connection):
    await some_async_operation()

@echo.receive
async def on_message(conn: Connection, data: str | bytes):
    result = await fetch_data(data)
    await conn.asend(result)
```

## Multiple Routes

Create multiple routes on the same server:

```python
server = WebsocketServer(host="0.0.0.0", port=8080)

chat = server.create_route("ws/chat/", "chat")

@chat.receive
def on_chat(conn: Connection, data: str | bytes):
    conn.broadcast(["chat"], data)

notifications = server.create_route("ws/notifications/", "notifications")

@notifications.receive
def on_notification(conn: Connection, data: str | bytes):
    conn.broadcast(["notifications"], data)

server.start()
```

## Next Steps

- [Django Integration](/guides/django/) - Use webrockets with Django
- [Pattern Matching](/guides/pattern-matching/) - Route messages by JSON fields
- [Broadcasting](/guides/broadcasting/) - Send messages to all clients
