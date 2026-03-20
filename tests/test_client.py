import asyncio

import pytest
from webrockets import Connection, IncomingConnection
from webrockets.client import ClientConfig, aconnect, connect
from webrockets.test import runserver


class TestClient:
    def test_client_connect_timeout(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("before")
        async def conn(conn: IncomingConnection):
            await asyncio.sleep(501)

        with runserver(ws_server):
            try:
                with connect(f"ws://{ws_server.addr()}/ws", timeout=500):
                    pytest.fail("Expected connect to timeout")
            except TimeoutError as e:
                assert "timed out" in str(e)

    @pytest.mark.asyncio
    async def test_async_client_connect_timeout(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("before")
        async def conn(conn: IncomingConnection):
            await asyncio.sleep(501)

        with runserver(ws_server):
            try:
                async with aconnect(f"ws://{ws_server.addr()}/ws", timeout=500):
                    pytest.fail("Expected connect to timeout")
            except TimeoutError as e:
                assert "timed out" in str(e)

    def test_client_receive_timeout(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            pass

        with runserver(ws_server):
            with connect(f"ws://{ws_server.addr()}/ws") as ws:
                try:
                    ws.recv(timeout=500)
                    pytest.fail("Expected receive to timeout")
                except TimeoutError as e:
                    assert "timed out" in str(e)

    @pytest.mark.asyncio
    async def test_async_client_receive_timeout(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            pass

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                try:
                    await ws.recv(timeout=500)
                    pytest.fail("Expected receive to timeout")
                except TimeoutError as e:
                    assert "timed out" in str(e)

    @pytest.mark.asyncio
    async def test_async_client_max_message_size(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            conn.send("A" * (1 << 21))

        with runserver(ws_server):
            async with aconnect(
                f"ws://{ws_server.addr()}/ws",
                config=ClientConfig(max_message_size=1 << 20),  # 1 MiB
            ) as ws:
                try:
                    await ws.recv(timeout=500)
                    pytest.fail("Expected frame too large")
                except RuntimeError as e:
                    assert "Frame too large" in str(e)


class TestSubprotocolNegotiation:
    @pytest.mark.asyncio
    async def test_hook_negotiates_subprotocol(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("before")
        def on_connect(conn: IncomingConnection):
            requested = conn.get_header("sec-websocket-protocol") or ""
            protocols = [p.strip() for p in requested.split(",") if p.strip()]
            if "graphql-ws" in protocols:
                conn.subprotocol = "graphql-ws"

        with runserver(ws_server):
            async with aconnect(
                f"ws://{ws_server.addr()}/ws",
                config=ClientConfig(subprotocols=["graphql-ws", "chat"]),
            ) as ws:
                assert ws.negotiated_protocol == "graphql-ws"

    @pytest.mark.asyncio
    async def test_subprotocol_propagates_to_connection(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("before")
        def on_connect(conn: IncomingConnection):
            conn.subprotocol = "chat"

        @route.connect("after")
        def on_connected(conn: Connection):
            conn.send(conn.subprotocol or "none")

        with runserver(ws_server):
            async with aconnect(
                f"ws://{ws_server.addr()}/ws",
                config=ClientConfig(subprotocols=["chat"]),
            ) as ws:
                assert ws.negotiated_protocol == "chat"
                msg = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert msg == "chat"

    @pytest.mark.asyncio
    async def test_no_subprotocol_when_hook_skips(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("before")
        def on_connect(conn: IncomingConnection):
            pass

        with runserver(ws_server):
            async with aconnect(
                f"ws://{ws_server.addr()}/ws",
                config=ClientConfig(subprotocols=["graphql-ws"]),
            ) as ws:
                assert ws.negotiated_protocol is None

    def test_sync_client_subprotocol(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("before")
        def on_connect(conn: IncomingConnection):
            conn.subprotocol = "chat"

        with runserver(ws_server):
            with connect(
                f"ws://{ws_server.addr()}/ws",
                config=ClientConfig(subprotocols=["chat"]),
            ) as ws:
                assert ws.negotiated_protocol == "chat"


class TestSSL:
    def test_verify_ssl_rejects_plain_server(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            pass

        with runserver(ws_server):
            with pytest.raises(OSError):
                connect(
                    f"wss://{ws_server.addr()}/ws",
                    config=ClientConfig(verify_ssl=True),
                )

    @pytest.mark.asyncio
    async def test_async_verify_ssl_rejects_plain_server(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        async def conn(conn: Connection):
            pass

        with runserver(ws_server):
            with pytest.raises(OSError):
                async with aconnect(
                    f"wss://{ws_server.addr()}/ws",
                    config=ClientConfig(verify_ssl=True),
                ):
                    pass
