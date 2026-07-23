import asyncio

import pytest
from webrockets import Connection, IncomingConnection
from webrockets.client import ClientConfig, ConnectionClosed, aconnect, connect
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


class TestIteration:
    def test_sync_client_iterates_messages(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            conn.send("one")
            conn.send("two")
            conn.close()

        with runserver(ws_server):
            with connect(f"ws://{ws_server.addr()}/ws") as ws:
                assert iter(ws) is ws
                msgs = list(ws)

        assert msgs == ["one", "two"]

    @pytest.mark.asyncio
    async def test_async_client_iterates_messages(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            conn.send("one")
            conn.send("two")
            conn.close()

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                assert ws.__aiter__() is ws
                msgs = [msg async for msg in ws]

        assert msgs == ["one", "two"]

    def test_sync_client_iteration_stops_on_normal_close(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            conn.send("hello")
            conn.close(code=1000)

        with runserver(ws_server):
            with connect(f"ws://{ws_server.addr()}/ws") as ws:
                msgs = []
                for msg in ws:
                    msgs.append(msg)

        assert msgs == ["hello"]

    @pytest.mark.asyncio
    async def test_async_client_iteration_stops_on_normal_close(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            conn.send("hello")
            conn.close(code=1000)

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                msgs = []
                async for msg in ws:
                    msgs.append(msg)

        assert msgs == ["hello"]

    def test_sync_client_iteration_raises_on_abnormal_close(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            conn.send("hello")
            conn.close(code=1011, reason="oops")

        with runserver(ws_server):
            with connect(f"ws://{ws_server.addr()}/ws") as ws:
                msgs = []
                with pytest.raises(ConnectionClosed) as exc_info:
                    for msg in ws:
                        msgs.append(msg)

        assert msgs == ["hello"]
        assert exc_info.value.code == 1011
        assert exc_info.value.reason == "oops"

    @pytest.mark.asyncio
    async def test_async_client_iteration_raises_on_abnormal_close(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("after")
        def conn(conn: Connection):
            conn.send("hello")
            conn.close(code=1011, reason="oops")

        with runserver(ws_server):
            async with aconnect(f"ws://{ws_server.addr()}/ws") as ws:
                msgs = []
                with pytest.raises(ConnectionClosed) as exc_info:
                    async for msg in ws:
                        msgs.append(msg)

        assert msgs == ["hello"]
        assert exc_info.value.code == 1011
        assert exc_info.value.reason == "oops"


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
