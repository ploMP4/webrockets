import asyncio
import json
import os
import threading
import time

import pytest
import websockets
from pydantic import BaseModel
from pywsrs import IncomingConnection, Match, Websocket

test_state = {
    "connected": [],
    "messages": [],
    "disconnected": [],
}

pattern_state = {
    "chat_messages": [],
    "join_events": [],
    "leave_events": [],
    "unknown_messages": [],
    "ping_events": [],
    "pong_events": [],
}


class ChatMessage(BaseModel):
    type: str
    content: str
    room: str


class JoinRoom(BaseModel):
    type: str
    room: str
    username: str


class LeaveRoom(BaseModel):
    type: str
    room: str


class PingMessage(BaseModel):
    action: str
    timestamp: int


class PongMessage(BaseModel):
    action: str
    timestamp: int


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
        conn.broadcast(["chat"], data)
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

    # Pattern matching routes
    pattern_view = Websocket("ws/pattern/", "pattern")

    @pattern_view.receive(match=Match("type", "chat"), schema=ChatMessage)
    def on_chat(conn, data: ChatMessage):
        pattern_state["chat_messages"].append(
            {
                "content": data.content,
                "room": data.room,
            }
        )
        conn.send(json.dumps({"status": "received", "type": "chat"}))

    @pattern_view.receive(match=Match("type", "join"), schema=JoinRoom)
    def on_join(conn, data: JoinRoom):
        pattern_state["join_events"].append(
            {
                "room": data.room,
                "username": data.username,
            }
        )
        conn.send(json.dumps({"status": "joined", "room": data.room}))

    @pattern_view.receive(match=Match("type", "leave"), schema=LeaveRoom)
    def on_leave(conn, data: LeaveRoom):
        pattern_state["leave_events"].append({"room": data.room})
        conn.send(json.dumps({"status": "left", "room": data.room}))

    @pattern_view.receive
    def on_fallback(conn, data):
        pattern_state["unknown_messages"].append(data)
        conn.send(json.dumps({"status": "unknown", "raw": str(data)[:100]}))

    # Custom discriminator test (using "action" key instead of "type")
    custom_view = Websocket("ws/custom-disc/", "custom-disc")

    @custom_view.receive(match=Match("action", "ping"), schema=PingMessage)
    def on_ping(conn, data: PingMessage):
        pattern_state["ping_events"].append({"timestamp": data.timestamp})
        conn.send(json.dumps({"action": "pong", "timestamp": data.timestamp}))

    @custom_view.receive(match=Match("action", "pong"), schema=PongMessage)
    def on_pong(conn, data: PongMessage):
        pattern_state["pong_events"].append({"timestamp": data.timestamp})
        conn.send(json.dumps({"status": "pong_received"}))

    # Raw match without schema
    raw_view = Websocket("ws/raw-match/", "raw-match")

    @raw_view.receive(match=Match("type", "echo"))
    def on_echo_raw(conn, data: str):
        conn.send(f"raw: {data}")

    @raw_view.receive
    def on_raw_fallback(conn, data):
        conn.send(f"fallback: {data}")


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
    time.sleep(0.5)  # Wait for server to start

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
    pattern_state["chat_messages"] = []
    pattern_state["join_events"] = []
    pattern_state["leave_events"] = []
    pattern_state["unknown_messages"] = []
    pattern_state["ping_events"] = []
    pattern_state["pong_events"] = []
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

        for _ in range(num_clients):
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

        await asyncio.gather(*[send_and_receive(f"{ws_server}/ws/echo/", i, 5) for i in range(5)])


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
                pytest.fail("Expected connection to close")
            except websockets.ConnectionClosedOK as e:
                assert e.rcvd.code == 1000
                assert e.rcvd.reason == ""

    @pytest.mark.asyncio
    async def test_server_close_with_code(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/close/") as ws:
            await ws.send("close:1001")

            try:
                await asyncio.wait_for(ws.recv(), timeout=2.0)
                pytest.fail("Expected connection to close")
            except websockets.ConnectionClosed as e:
                assert e.rcvd.code == 1001

    @pytest.mark.asyncio
    async def test_server_close_with_code_and_reason(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/close/") as ws:
            await ws.send("close:1008:Policy violation")

            try:
                await asyncio.wait_for(ws.recv(), timeout=2.0)
                pytest.fail("Expected connection to close")
            except websockets.ConnectionClosed as e:
                assert e.rcvd.code == 1008
                assert e.rcvd.reason == "Policy violation"

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
                pytest.fail("Expected connection to close")
            except websockets.ConnectionClosedOK as e:
                assert e.rcvd.code == 1000
                assert e.rcvd.reason == "Goodbye"


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


class TestPatternMatching:
    @pytest.mark.asyncio
    async def test_match_chat_message(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/pattern/") as ws:
            msg = {"type": "chat", "content": "Hello!", "room": "general"}
            await ws.send(json.dumps(msg))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert json.loads(response) == {"status": "received", "type": "chat"}
            assert len(pattern_state["chat_messages"]) == 1
            assert pattern_state["chat_messages"][0]["content"] == "Hello!"
            assert pattern_state["chat_messages"][0]["room"] == "general"

    @pytest.mark.asyncio
    async def test_match_join_message(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/pattern/") as ws:
            msg = {"type": "join", "room": "lobby", "username": "alice"}
            await ws.send(json.dumps(msg))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert json.loads(response) == {"status": "joined", "room": "lobby"}
            assert len(pattern_state["join_events"]) == 1
            assert pattern_state["join_events"][0]["username"] == "alice"

    @pytest.mark.asyncio
    async def test_match_leave_message(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/pattern/") as ws:
            msg = {"type": "leave", "room": "lobby"}
            await ws.send(json.dumps(msg))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert json.loads(response) == {"status": "left", "room": "lobby"}
            assert len(pattern_state["leave_events"]) == 1

    @pytest.mark.asyncio
    async def test_fallback_for_unknown_type(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/pattern/") as ws:
            msg = {"type": "unknown_action", "data": "test"}
            await ws.send(json.dumps(msg))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            resp_data = json.loads(response)
            assert resp_data["status"] == "unknown"
            assert len(pattern_state["unknown_messages"]) == 1

    @pytest.mark.asyncio
    async def test_fallback_for_non_json(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/pattern/") as ws:
            await ws.send("not valid json")
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            resp_data = json.loads(response)
            assert resp_data["status"] == "unknown"
            assert "not valid json" in resp_data["raw"]

    @pytest.mark.asyncio
    async def test_multiple_messages_different_types(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/pattern/") as ws:
            # Send chat message
            await ws.send(json.dumps({"type": "chat", "content": "msg1", "room": "r1"}))
            await asyncio.wait_for(ws.recv(), timeout=2.0)

            # Send join
            await ws.send(json.dumps({"type": "join", "room": "r2", "username": "bob"}))
            await asyncio.wait_for(ws.recv(), timeout=2.0)

            # Send another chat
            await ws.send(json.dumps({"type": "chat", "content": "msg2", "room": "r1"}))
            await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert len(pattern_state["chat_messages"]) == 2
            assert len(pattern_state["join_events"]) == 1


class TestCustomDiscriminator:
    @pytest.mark.asyncio
    async def test_custom_discriminator_ping(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/custom-disc/") as ws:
            msg = {"action": "ping", "timestamp": 12345}
            await ws.send(json.dumps(msg))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            resp_data = json.loads(response)
            assert resp_data["action"] == "pong"
            assert resp_data["timestamp"] == 12345
            assert len(pattern_state["ping_events"]) == 1

    @pytest.mark.asyncio
    async def test_custom_discriminator_pong(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/custom-disc/") as ws:
            msg = {"action": "pong", "timestamp": 67890}
            await ws.send(json.dumps(msg))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert json.loads(response) == {"status": "pong_received"}
            assert len(pattern_state["pong_events"]) == 1


class TestRawMatchWithoutSchema:
    @pytest.mark.asyncio
    async def test_raw_match_receives_json_string(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/raw-match/") as ws:
            msg = {"type": "echo", "data": "test123"}
            await ws.send(json.dumps(msg))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            # Handler receives raw JSON string, not parsed object
            assert response.startswith("raw: ")
            assert "echo" in response
            assert "test123" in response

    @pytest.mark.asyncio
    async def test_raw_fallback(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/raw-match/") as ws:
            await ws.send("plain text message")
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert response == "fallback: plain text message"


class TestPydanticValidation:
    @pytest.mark.asyncio
    async def test_extra_fields_accepted(self, ws_server):
        async with websockets.connect(f"{ws_server}/ws/pattern/") as ws:
            # Pydantic v2 ignores extra fields by default
            msg = {"type": "chat", "content": "Hi", "room": "test", "extra": "ignored"}
            await ws.send(json.dumps(msg))
            response = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert json.loads(response) == {"status": "received", "type": "chat"}
            assert len(pattern_state["chat_messages"]) == 1
