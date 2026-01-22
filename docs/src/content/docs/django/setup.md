---
title: Setup
description: Set up webrockets with your Django application.
---

webrockets provides first-class Django integration with built-in authentication, settings configuration, and route autodiscovery.

## Installation

Install webrockets with Django extras:

```bash
pip install webrockets[django]
```

For Pydantic schema validation:

```bash
pip install webrockets[schema,django]
```

## Configuration

### Add to INSTALLED_APPS

Add `webrockets` to your Django `INSTALLED_APPS`:

```python
# settings.py
INSTALLED_APPS = [
    # ...
    "webrockets",
]
```

### Settings

Configure the WebSocket server in your Django settings:

```python
# settings.py

# Server host (default: "0.0.0.0")
WEBSOCKET_HOST = "0.0.0.0"

# Server port (default: 46290)
WEBSOCKET_PORT = 46290

# Message broker for multi-server broadcasting (default: None)
# See the Brokers & Broadcasting guide for options
WEBSOCKET_BROKER = None
```

## Creating Routes

Create a `websockets.py` file in any of your Django apps:

```python
# myapp/websockets.py
from webrockets.django import server
from webrockets.django.auth import SessionAuthentication

# Create a route with authentication
chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[SessionAuthentication()]
)

@chat.connect("before")
def on_connect(conn):
    # conn.user is set by SessionAuthentication
    print(f"User {conn.user} joined the chat")

@chat.receive
def on_message(conn, data):
    conn.send(f"{conn.user}: {data}")
    conn.broadcast(["chat"], data)

@chat.disconnect
def on_disconnect(conn, code=None, reason=None):
    print(f"User {conn.user} left")
```

## Running the Server

Start the WebSocket server using the management command:

```bash
python manage.py runwebsockets
```

With logging enabled:

```bash
python manage.py runwebsockets --log-level info
```

:::note
The WebSocket server runs as a **separate process** from your Django application. You'll need to run both your Django server and the WebSocket server.
:::

## Route Autodiscovery

The `runwebsockets` command automatically discovers routes from these modules in your Django apps:

- `websockets.py`
- `sockets.py`
- `views.py`

Place your route definitions in any of these files and they will be automatically loaded.

## Accessing Request Data

The `conn` object provides access to the original request:

```python
@chat.connect("before")
def on_connect(conn):
    # User (set by authentication)
    user = conn.user

    # Request path
    path = conn.path  # "/ws/chat/"

    # Query string
    query = conn.query_string  # "room=general&token=abc"

    # Get specific cookie
    session_id = conn.get_cookie("sessionid")

    # Get specific header
    origin = conn.get_header("origin")
```

## Project Structure

A typical Django project with webrockets:

```
myproject/
├── myproject/
│   ├── settings.py
│   └── urls.py
├── chat/
│   ├── models.py
│   ├── views.py
│   └── websockets.py    # WebSocket routes
├── notifications/
│   └── websockets.py    # More routes
└── manage.py
```

## Next Steps

- [Authentication](/django/authentication/) - Configure user authentication
- [Broadcasting](/django/broadcasting/) - Send messages from Django to clients
- [Deployment](/django/deployment/overview/) - Deploy to production
