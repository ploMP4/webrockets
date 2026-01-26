---
title: Testing
description: How to test your WebSocket applications with pytest.
---

This guide covers testing WebSocket applications using pytest and the `runserver` context manager.

## Setup

Install the required dependencies:

```bash
pip install pytest pytest-asyncio
```

## Project Structure

Assuming you have a server module with routes defined:

```python
# myapp/websockets.py
from webrockets import WebsocketServer, Connection

server = WebsocketServer(host="0.0.0.0", port=8080)

echo = server.create_route("ws/echo/")

@echo.receive
def on_message(conn: Connection, data: str | bytes):
    conn.send(f"echo: {data}")


chat = server.create_route("ws/chat/")

@chat.connect("after")
def on_connect(conn: Connection):
    conn.join("room")

@chat.receive
def on_message(conn: Connection, data: str | bytes):
    conn.broadcast(["room"], data)
```

## Basic Testing

Use the `runserver` context manager to start and stop the server during tests:

```python
# tests/test_websockets.py
from webrockets.client import connect
from webrockets.test import runserver

from myapp.websockets import server


class TestEchoServer:
    def test_echo(self):
        with runserver(server):
            with connect(f"ws://{server.addr()}/ws/echo/") as ws:
                ws.send("hello")
                response = ws.recv()
                assert response == "echo: hello"
```

## Async Testing

For async tests, use `pytest-asyncio` and the async webrockets client:

```python
import asyncio
import pytest
from webrockets.client import aconnect
from webrockets.test import runserver

from myapp.websockets import server


class TestAsyncServer:
    @pytest.mark.asyncio
    async def test_async_echo(self):
        with runserver(server):
            async with aconnect(f"ws://{server.addr()}/ws/echo/") as ws:
                await ws.send("hello")
                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert response == "echo: hello"
```

## Testing Multiple Clients

Test broadcasting and multi-client scenarios:

```python
from webrockets.client import connect
from webrockets.test import runserver

from myapp.websockets import server


def test_broadcast():
    with runserver(server):
        with connect(f"ws://{server.addr()}/ws/chat/") as ws1:
            with connect(f"ws://{server.addr()}/ws/chat/") as ws2:
                ws1.send("hello everyone")

                # Both clients receive the broadcast
                assert ws1.recv() == "hello everyone"
                assert ws2.recv() == "hello everyone"
```

## Next Steps

- [Debugging](/guides/debugging/) - Debug WebSocket callbacks with remote-pdb
- [Django Testing](/django/testing/) - Test Django WebSocket applications
