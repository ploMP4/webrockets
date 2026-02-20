import asyncio

import pytest
from webrockets import Connection, IncomingConnection
from webrockets.client import aconnect, connect
from webrockets.test import runserver


class TestClient:
    def test_client_connect_timeout(self, ws_server):
        route = ws_server.create_route("ws")

        @route.connect("before")
        async def conn(conn: IncomingConnection):
            await asyncio.sleep(2)

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
            await asyncio.sleep(2)

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
