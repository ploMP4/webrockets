import asyncio
import os
import threading
from django_wsrs.django_wsrs import IncomingConnection
import pytest
import websockets

from django_wsrs import Websocket

test_state = {
    "connected": [],
    "messages": [],
    "disconnected": [],
}


def setup_routes():
    echo_view = Websocket("ws/echo/", "echo")

    @echo_view.connect("before")
    def echo_connect(conn: IncomingConnection):
        test_state["connected"].append(("echo", conn.path))

    @echo_view.receive
    def echo_receive(conn, data):
        conn.send(f"echo: {data}")
        test_state["messages"].append(("echo", data))

    @echo_view.disconnect
    def echo_disconnect(conn, code=None, reason=None):
        test_state["disconnected"].append(("echo", code, reason))

    chat_view = Websocket("ws/chat/", "chat")

    @chat_view.connect("before")
    def chat_connect(conn):
        test_state["connected"].append(("chat", conn.path))

    @chat_view.receive
    def chat_receive(conn, data):
        Websocket.broadcast_text(["chat"], data)
        test_state["messages"].append(("chat", data))

    @chat_view.disconnect
    def chat_disconnect(conn, code=None, reason=None):
        test_state["disconnected"].append(("chat", code, reason))

    binary_view = Websocket("ws/binary/", "binary")

    @binary_view.connect("before")
    def binary_connect(conn):
        test_state["connected"].append(("binary", conn.path))

    @binary_view.receive
    def binary_receive(conn, data):
        conn.send(data)
        test_state["messages"].append(("binary", type(data).__name__, data))

    @binary_view.disconnect
    def binary_disconnect(conn, code=None, reason=None):
        test_state["disconnected"].append(("binary", code, reason))

    close_view = Websocket("ws/close/", "close")

    @close_view.connect("before")
    def close_connect(conn):
        test_state["connected"].append(("close", conn.path))

    @close_view.receive
    def close_receive(conn, data):
        # Parse close commands: "close", "close:1001", "close:1008:reason"
        if data == "close":
            conn.close()
        elif data.startswith("close:"):
            parts = data.split(":", 2)
            code = int(parts[1])
            reason = parts[2] if len(parts) > 2 else ""
            conn.close(code, reason)
        else:
            conn.send(data)

    @close_view.disconnect
    def close_disconnect(conn, code=None, reason=None):
        test_state["disconnected"].append(("close", code, reason))

    async_view = Websocket("ws/async/", "async")

    @async_view.connect("before")
    async def async_connect(conn):
        await asyncio.sleep(0.1)
        test_state["connected"].append(("async", conn.path))

    @async_view.receive
    async def async_receive(conn, data):
        await asyncio.sleep(0.1)
        await conn.asend(data)

    @async_view.disconnect
    async def async_disconnect(conn, code=None, reason=None):
        await asyncio.sleep(0.1)
        test_state["disconnected"].append(("async", code, reason))


@pytest.fixture(scope="module")
def ws_server():
    os.environ.setdefault("DJANGO_SETTINGS_MODULE", "tests.settings")

    from django.conf import settings

    if not hasattr(settings, "WEBSOCKET_PORT"):
        settings.WEBSOCKET_HOST = "127.0.0.1"
        settings.WEBSOCKET_PORT = "46290"

    setup_routes()

    server_thread = threading.Thread(target=Websocket.start)
    server_thread.start()

    yield "ws://127.0.0.1:46290"

    Websocket.stop()
    server_thread.join(timeout=5)
    if server_thread.is_alive():
        raise RuntimeError("WebSocket server did not shut down cleanly")


@pytest.fixture(autouse=True)
def cleanup_state():
    test_state["connected"] = []
    test_state["messages"] = []
    test_state["disconnected"] = []
    yield


class TestEchoServer:
    @pytest.mark.asyncio
    async def test_connect_and_receive_echo(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/echo/") as ws:
            await ws.send("hello")
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert response == "echo: hello"

    @pytest.mark.asyncio
    async def test_multiple_messages(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/echo/") as ws:
            messages = ["first", "second", "third"]

            for msg in messages:
                await ws.send(msg)
                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert response == f"echo: {msg}"

    @pytest.mark.asyncio
    async def test_echo_preserves_content(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/echo/") as ws:
            test_messages = [
                "Hello, World!",
                '{"json": "data", "num": 123}',
                "Unicode: 你好世界 🚀",
                "Special: <>&\"'",
            ]

            for msg in test_messages:
                await ws.send(msg)
                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert response == f"echo: {msg}"


class TestChatBroadcast:
    @pytest.mark.asyncio
    async def test_broadcast_to_multiple_clients(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/chat/") as ws1:
            async with websockets.connect(f"{ws_server}/ws/chat/") as ws2:
                await asyncio.sleep(0.1)

                await ws1.send("Hello everyone!")

                response1 = await asyncio.wait_for(ws1.recv(), timeout=2.0)
                response2 = await asyncio.wait_for(ws2.recv(), timeout=2.0)

                assert response1 == "Hello everyone!"
                assert response2 == "Hello everyone!"

    @pytest.mark.asyncio
    async def test_broadcast_from_different_senders(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/chat/") as ws1:
            async with websockets.connect(f"{ws_server}/ws/chat/") as ws2:
                await asyncio.sleep(0.1)

                await ws1.send("From client 1")
                r1 = await asyncio.wait_for(ws1.recv(), timeout=2.0)
                r2 = await asyncio.wait_for(ws2.recv(), timeout=2.0)
                assert r1 == r2 == "From client 1"

                await ws2.send("From client 2")
                r1 = await asyncio.wait_for(ws1.recv(), timeout=2.0)
                r2 = await asyncio.wait_for(ws2.recv(), timeout=2.0)
                assert r1 == r2 == "From client 2"


class TestConnectionLifecycle:
    @pytest.mark.asyncio
    async def test_connect_callback_triggered(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/echo/"):
            assert len(test_state["connected"]) >= 1
            assert any(c[0] == "echo" for c in test_state["connected"])

    @pytest.mark.asyncio
    async def test_disconnect_callback_triggered(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/echo/") as ws:
            await ws.send("test")
            await ws.recv()

        assert len(test_state["disconnected"]) >= 1
        assert any(d[0] == "echo" for d in test_state["disconnected"])

    @pytest.mark.asyncio
    async def test_multiple_connect_disconnect(self, ws_server):
        initial_connected = len(test_state["connected"])

        for i in range(3):
            async with websockets.connect(f"{ws_server}/ws/echo/") as ws:
                await ws.send(f"msg{i}")
                await ws.recv()

        assert len(test_state["connected"]) >= initial_connected + 3


class TestConcurrentConnections:
    @pytest.mark.asyncio
    async def test_many_concurrent_connections(self, ws_server):
        num_clients = 10
        clients = []

        for i in range(num_clients):
            ws = await websockets.connect(f"{ws_server}/ws/echo/")
            clients.append(ws)

        for i, ws in enumerate(clients):
            await ws.send(f"client_{i}")

        for i, ws in enumerate(clients):
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)
            assert response == f"echo: client_{i}"

        for ws in clients:
            await ws.close()

    @pytest.mark.asyncio
    async def test_concurrent_message_handling(self, ws_server):
        async def send_and_receive(url, client_id, num_msgs):
            async with websockets.connect(url) as ws:
                for i in range(num_msgs):
                    await ws.send(f"c{client_id}m{i}")
                    response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                    assert response == f"echo: c{client_id}m{i}"

        await asyncio.gather(
            *[send_and_receive(f"{ws_server}/ws/echo/", i, 5) for i in range(5)]
        )


class TestMessageTypes:
    @pytest.mark.asyncio
    async def test_json_messages(self, ws_server):
        import json

        async with websockets.connect(f"{ws_server}/ws/echo/") as ws:
            data = {"action": "subscribe", "channel": "updates", "id": 42}
            await ws.send(json.dumps(data))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert response == f"echo: {json.dumps(data)}"

    @pytest.mark.asyncio
    async def test_empty_message(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/echo/") as ws:
            await ws.send("")
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)
            assert response == "echo: "

    @pytest.mark.asyncio
    async def test_large_message(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/echo/") as ws:
            large_msg = "x" * 10000
            await ws.send(large_msg)
            response = await asyncio.wait_for(ws.recv(), timeout=5.0)
            assert response == f"echo: {large_msg}"


class TestBinaryMessages:
    @pytest.mark.asyncio
    async def test_send_and_receive_binary(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/binary/") as ws:
            binary_data = b"\x00\x01\x02\x03\xff\xfe"
            await ws.send(binary_data)
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert response == binary_data
            assert isinstance(response, bytes)

    @pytest.mark.asyncio
    async def test_binary_preserves_content(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/binary/") as ws:
            # Test various binary patterns
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

    @pytest.mark.asyncio
    async def test_text_still_works_on_binary_endpoint(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/binary/") as ws:
            await ws.send("hello text")
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert response == "hello text"
            assert isinstance(response, str)

    @pytest.mark.asyncio
    async def test_callback_receives_correct_types(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/binary/") as ws:
            await ws.send("text message")
            await ws.recv()

            await ws.send(b"\x00\x01\x02")
            await ws.recv()

        binary_messages = [m for m in test_state["messages"] if m[0] == "binary"]
        assert len(binary_messages) >= 2

        assert binary_messages[-2][1] == "str"
        assert binary_messages[-2][2] == "text message"

        assert binary_messages[-1][1] == "bytes"
        assert binary_messages[-1][2] == b"\x00\x01\x02"

    @pytest.mark.asyncio
    async def test_large_binary_message(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/binary/") as ws:
            large_binary = bytes(range(256)) * 100  # 25.6 KB
            await ws.send(large_binary)
            response = await asyncio.wait_for(ws.recv(), timeout=5.0)

            assert response == large_binary
            assert isinstance(response, bytes)


class TestServerClose:
    @pytest.mark.asyncio
    async def test_server_close_default(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/close/") as ws:
            await ws.send("close")

            try:
                await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert False, "Expected connection to close"
            except websockets.ConnectionClosedOK as e:
                assert e.code == 1000
                assert e.reason == ""

    @pytest.mark.asyncio
    async def test_server_close_with_code(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/close/") as ws:
            await ws.send("close:1001")

            try:
                await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert False, "Expected connection to close"
            except websockets.ConnectionClosed as e:
                assert e.code == 1001

    @pytest.mark.asyncio
    async def test_server_close_with_code_and_reason(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/close/") as ws:
            await ws.send("close:1008:Policy violation")

            try:
                await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert False, "Expected connection to close"
            except websockets.ConnectionClosed as e:
                assert e.code == 1008
                assert e.reason == "Policy violation"

    @pytest.mark.asyncio
    async def test_messages_before_close(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/close/") as ws:
            await ws.send("hello")
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)
            assert response == "hello"

            await ws.send("world")
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)
            assert response == "world"

            await ws.send("close:1000:Goodbye")

            try:
                await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert False, "Expected connection to close"
            except websockets.ConnectionClosedOK as e:
                assert e.code == 1000
                assert e.reason == "Goodbye"


class TestAsyncCallbacks:
    @pytest.mark.asyncio
    async def test_async_callback_triggered(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/async/") as ws:
            await ws.send("test")
            msg = await ws.recv()
            assert msg == "test"

        await asyncio.sleep(0.5)

        assert len(test_state["connected"]) >= 1
        assert any(c[0] == "async" for c in test_state["connected"])
        assert len(test_state["disconnected"]) >= 1
        assert any(d[0] == "async" for d in test_state["disconnected"])
