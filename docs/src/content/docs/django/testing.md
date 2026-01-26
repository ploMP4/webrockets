---
title: Testing
description: How to test Django WebSocket applications with WebsocketTestCase.
---

This guide covers testing Django WebSocket applications using the `WebsocketTestCase` class.

## WebsocketTestCase

`WebsocketTestCase` extends Django's `TestCase` and automatically manages the WebSocket server lifecycle:

```python
from webrockets.django import server
from webrockets.django.test import WebsocketTestCase
from webrockets.client import connect


class MyChatTests(WebsocketTestCase):
    def test_echo(self):
        with connect(f"ws://{server.addr()}/ws/echo/") as ws:
            ws.send("hello")
            response = ws.receive()
            self.assertEqual(response, "echo: hello")
```

The server starts automatically in `setUpClass` and stops in `tearDownClass`.

## Full Example

Here's a complete example testing a chat application:

```python
# chat/websockets.py
import json
from webrockets.django import server

chat = server.create_route("ws/chat/", "chat")

@chat.connect("after")
def on_connect(conn):
    conn.send(json.dumps({"type": "welcome"}))

@chat.receive(match=Match("type", "join"))
def on_join(conn, data):
    msg = json.loads(data)
    conn.join(msg["room"])
    conn.send(json.dumps({"type": "joined", "room": msg["room"]}))


@chat.receive(match=Match("type", "message"))
def on_message(conn, data):
    conn.broadcast(conn.groups(), data)
```

```python
# chat/tests.py
import json

from webrockets.client import connect
from webrockets.django import server
from webrockets.django.test import WebsocketTestCase


class ChatTests(WebsocketTestCase):
    def test_connect_receives_welcome(self):
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            response = json.loads(ws.receive())
            self.assertEqual(response["type"], "welcome")

    def test_join_room(self):
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            ws.receive()  # welcome
            ws.send(json.dumps({"type": "join", "room": "general"}))
            response = json.loads(ws.receive())
            self.assertEqual(response["type"], "joined")
            self.assertEqual(response["room"], "general")

    def test_broadcast_message(self):
        with connect(f"ws://{server.addr()}/ws/chat/") as ws1:
            with connect(f"ws://{server.addr()}/ws/chat/") as ws2:
                # Both join the same room
                ws1.receive()  # welcome
                ws2.receive()  # welcome

                ws1.send(json.dumps({"type": "join", "room": "test"}))
                ws1.receive()  # joined

                ws2.send(json.dumps({"type": "join", "room": "test"}))
                ws2.receive()  # joined

                # Send a message
                msg = json.dumps({"type": "message", "text": "hello"})
                ws1.send(msg)

                # Both receive the broadcast
                self.assertEqual(ws1.receive(), msg)
                self.assertEqual(ws2.receive(), msg)
```

## Running Tests

Run with Django's test runner:

```bash
python manage.py test yourapp.tests
```

Or with pytest-django:

```bash
pytest yourapp/tests.py
```

## Testing with Authentication

Test authenticated WebSocket connections:

```python
from django.contrib.auth import get_user_model
from webrockets.client import connect, ClientConfig
from webrockets.django import server
from webrockets.django.test import WebsocketTestCase

User = get_user_model()


class AuthenticatedTests(WebsocketTestCase):
    def setUp(self):
        self.user = User.objects.create_user(
            username="testuser",
            password="testpass"
        )
        # Create a session for the user
        from django.contrib.sessions.backends.db import SessionStore
        session = SessionStore()
        session["_auth_user_id"] = str(self.user.pk)
        session["_auth_user_backend"] = "django.contrib.auth.backends.ModelBackend"
        session.create()
        self.session_key = session.session_key

    def test_authenticated_connection(self):
        with connect(
            f"ws://{server.addr()}/ws/protected/",
            config=ClientConfig(extra_headers={"Cookie": f"sessionid={self.session_key}"})
        ) as ws:
            response = ws.receive()
            self.assertIn(self.user.username, response)
```

## Next Steps

- [Debugging](/guides/debugging/) - Debug WebSocket callbacks with remote-pdb
- [Authentication](/django/authentication/) - Secure your WebSocket routes
