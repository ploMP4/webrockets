import asyncio
import json

import pytest
from pydantic import BaseModel
from webrockets import Connection, IncomingConnection, Match, WebsocketServer
from webrockets.client import ConnectionClosed, aconnect
from webrockets.test import runserver


class TestCallbacks:
    @pytest.mark.asyncio
    async def test_connect(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.connect("before")
        def before_connect(conn: IncomingConnection):
            assert isinstance(conn, IncomingConnection)
            assert conn.path == "/ws"

        @route.connect("after")
        def after_connect(conn: Connection):
            assert isinstance(conn, Connection)
            assert conn.path == "/ws"
            conn.close(code=1000)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                try:
                    await asyncio.wait_for(ws.recv(), timeout=2.0)
                    pytest.fail("Expected connection to close")
                except ConnectionClosed as e:
                    assert e.code == 1000

    @pytest.mark.asyncio
    async def test_async_connect(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.connect("before")
        async def before_connect(conn: IncomingConnection):
            assert isinstance(conn, IncomingConnection)
            assert conn.path == "/ws"

        @route.connect("after")
        async def after_connect(conn: Connection):
            assert isinstance(conn, Connection)
            assert conn.path == "/ws"
            await conn.aclose(code=1000)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                try:
                    await asyncio.wait_for(ws.recv(), timeout=2.0)
                    pytest.fail("Expected connection to close")
                except ConnectionClosed as e:
                    assert e.code == 1000

    @pytest.mark.asyncio
    async def test_receive(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            assert isinstance(conn, Connection)
            assert data == "hi"
            conn.close(code=1000)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                try:
                    await ws.send("hi")
                    await asyncio.wait_for(ws.recv(), timeout=2.0)
                    pytest.fail("Expected connection to close")
                except ConnectionClosed as e:
                    assert e.code == 1000

    @pytest.mark.asyncio
    async def test_async_receive(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.receive
        async def receive(conn: Connection, data: str | bytes):
            assert isinstance(conn, Connection)
            assert data == "hi"
            await conn.aclose(code=1000)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                try:
                    await ws.send("hi")
                    await asyncio.wait_for(ws.recv(), timeout=2.0)
                    pytest.fail("Expected connection to close")
                except ConnectionClosed as e:
                    assert e.code == 1000

    @pytest.mark.asyncio
    async def test_disconnect(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.disconnect
        def disconnect(conn: Connection, code: int | None, reason: str | None):
            assert code == 1000
            assert reason == "test"

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.close(code=1000, reason="test")

    @pytest.mark.asyncio
    async def test_async_disconnect(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.disconnect
        async def disconnect(conn: Connection, code: int | None, reason: str | None):
            assert code == 1000
            assert reason == "test"

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.close(code=1000, reason="test")


class TestMessageSending:
    @pytest.mark.asyncio
    async def test_send(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            conn.send(data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.send("hi")
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == "hi"

    @pytest.mark.asyncio
    async def test_async_send(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.receive
        async def receive(conn: Connection, data: str | bytes):
            await conn.asend(data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.send("hi")
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == "hi"

    @pytest.mark.asyncio
    async def test_send_and_receive_binary(self, ws_server):
        route = ws_server.create_route("ws")

        @route.receive
        async def receive(conn: Connection, data: str | bytes):
            await conn.asend(data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                binary_data = b"\x00\x01\x02\x03\xff\xfe"
                await ws.send(binary_data)
                response = await asyncio.wait_for(ws.recv(), timeout=2.0)

                assert response == binary_data
                assert isinstance(response, bytes)

    @pytest.mark.asyncio
    async def test_binary_preserves_content(self, ws_server):
        route = ws_server.create_route("ws")

        @route.receive
        async def receive(conn: Connection, data: str | bytes):
            await conn.asend(data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                test_data = [
                    b"\x00" * 100,  # Null bytes
                    b"\xff" * 100,  # High bytes
                    bytes(range(256)),  # All byte values
                    b"mixed\x00binary\xffdata",  # Mixed content
                ]

                for data in test_data:
                    await ws.send(data)
                    response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                    assert response == data
                    assert isinstance(response, bytes)


class TestGroups:
    @pytest.mark.asyncio
    async def test_join(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def connect(conn: Connection):
            conn.join("chat")

        @route.receive
        def receive(conn: Connection, _):
            conn.send(json.dumps(conn.groups()))

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.send("hi")
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == '["chat"]'

    @pytest.mark.asyncio
    async def test_leave(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws", "chat")

        @route.connect("after")
        def connect(conn: Connection):
            conn.join("test")
            conn.leave("chat")

        @route.receive
        def receive(conn: Connection, _):
            conn.send(json.dumps(conn.groups()))

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.send("hi")
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == '["test"]'


class TestBroadcast:
    @pytest.mark.asyncio
    async def test_broadcast(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def connect(conn: Connection):
            conn.join("chat")

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            conn.broadcast(["chat"], data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                async with aconnect(f"ws://{ws_server.addr()}/ws") as ws2:
                    await ws.send("hi")

                    msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                    assert msg == "hi"

                    msg = await asyncio.wait_for(ws2.recv(), timeout=2.0)
                    assert msg == "hi"

    @pytest.mark.asyncio
    async def test_async_broadcast(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def connect(conn: Connection):
            conn.join("chat")

        @route.receive
        async def receive(conn: Connection, data: str | bytes):
            await conn.abroadcast(conn.groups(), data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                async with aconnect(f"ws://{ws_server.addr()}/ws") as ws2:
                    await ws.send("hi")

                    msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                    assert msg == "hi"

                    msg = await asyncio.wait_for(ws2.recv(), timeout=2.0)
                    assert msg == "hi"


class TestPatternMatching:
    @pytest.mark.asyncio
    async def test_simple_pattern_match(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.receive(match=Match("type", "message"))
        async def receive(conn: Connection, data: str | bytes):
            await conn.asend(data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.send("hi")

                payload = json.dumps({"type": "message", "data": "hi"})
                await ws.send(payload)
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == payload

    @pytest.mark.asyncio
    async def test_match_on_multiple_keys(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.receive(match=Match(["type", "action"], "message"))
        async def receive(conn: Connection, data: str | bytes):
            await conn.asend(data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.send("hi")

                payload = json.dumps({"type": "message", "data": "hi"})
                await ws.send(payload)
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == payload

                payload = json.dumps({"action": "message", "data": "hi"})
                await ws.send(payload)
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == payload

    @pytest.mark.asyncio
    async def test_match_on_multiple_values(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.receive(match=Match("type", ["message", "mention"]))
        async def receive(conn: Connection, data: str | bytes):
            await conn.asend(data)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.send("hi")

                payload = json.dumps({"type": "message", "data": "hi"})
                await ws.send(payload)
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == payload

                payload = json.dumps({"type": "mention", "data": "hi"})
                await ws.send(payload)
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == payload

    @pytest.mark.asyncio
    async def test_generic_fallback(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws")

        @route.receive(match=Match("type", "message"))
        async def receive_message(conn: Connection, data: str | bytes):
            await conn.asend(data)

        @route.receive
        async def receive(conn: Connection, data: str | bytes):
            await conn.asend("generic")

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                await ws.send("hi")
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == "generic"

    @pytest.mark.asyncio
    async def test_match_remove_key(self, ws_server: WebsocketServer):
        class ChatPayload(BaseModel):
            content: str

        route = ws_server.create_route("ws")

        @route.receive(match=Match("type", "message", remove_key=True), schema=ChatPayload)
        async def receive_message(conn: Connection, data: ChatPayload):
            await conn.asend(data.model_dump_json())

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                payload = json.dumps({"type": "message", "content": "hi"})
                await ws.send(payload)
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == '{"content":"hi"}'
