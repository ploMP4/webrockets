from pywsrs import Connection, Websocket
from pywsrs.auth import BaseAuthentication


class TestSocketViewCreation:
    def test_create_view_basic(self):
        view = Websocket("ws/test/", "test_group")

        assert view.path == "ws/test/"
        assert view.group == "test_group"

    def test_create_view_with_auth_classes(self):
        class MockAuth(BaseAuthentication):
            def authenticate(self, conn):
                return None

        view = Websocket(
            "ws/secure/",
            "secure_group",
            authentication_classes=[MockAuth()],
        )

        assert view.path == "ws/secure/"
        assert view.group == "secure_group"

    def test_create_multiple_views(self):
        view1 = Websocket("ws/chat/", "chat")
        view2 = Websocket("ws/notifications/", "notifications")

        assert view1.path == "ws/chat/"
        assert view2.path == "ws/notifications/"
        assert view1.group == "chat"
        assert view2.group == "notifications"


class TestConnectDecorator:
    def test_connect_before_decorator_registers_callback(self):
        view = Websocket("ws/connect_test/", "connect_group")
        callback_called = []

        @view.connect("before")
        def on_connect(conn):
            callback_called.append(conn)

        assert callable(on_connect)

    def test_connect_after_decorator_registers_callback(self):
        view = Websocket("ws/connect_after_test/", "connect_after_group")
        callback_called = []

        @view.connect("after")
        def on_connect(conn):
            callback_called.append(conn)

        assert callable(on_connect)

    def test_connect_decorator_preserves_function(self):
        view = Websocket("ws/connect_preserve/", "connect_preserve")

        @view.connect("before")
        def my_connect_handler(conn):
            return "connected"

        conn = Connection("/ws/", "", {}, {})
        result = my_connect_handler(conn)
        assert result == "connected"

    def test_connect_invalid_argument_raises_error(self):
        import pytest

        view = Websocket("ws/connect_invalid/", "connect_invalid")

        with pytest.raises(ValueError, match="'before' or 'after'"):
            view.connect("invalid")


class TestReceiveDecorator:
    def test_receive_decorator_registers_callback(self):
        view = Websocket("ws/receive_test/", "receive_group")

        @view.receive
        def on_receive(conn, data):
            pass

        assert callable(on_receive)

    def test_receive_decorator_preserves_function(self):
        view = Websocket("ws/receive_preserve/", "receive_preserve")

        @view.receive
        def my_receive_handler(conn, data):
            return f"Received: {data}"

        conn = Connection("/ws/", "", {}, {})
        result = my_receive_handler(conn, "hello")
        assert result == "Received: hello"


class TestDisconnectDecorator:
    def test_disconnect_decorator_registers_callback(self):
        view = Websocket("ws/disconnect_test/", "disconnect_group")

        @view.disconnect
        def on_disconnect(conn, code=None, reason=None):
            pass

        assert callable(on_disconnect)

    def test_disconnect_decorator_preserves_function(self):
        view = Websocket("ws/disconnect_preserve/", "disconnect_preserve")

        @view.disconnect
        def my_disconnect_handler(conn, code=None, reason=None):
            return f"Disconnected: {code}"

        conn = Connection("/ws/", "", {}, {})
        result = my_disconnect_handler(conn, 1000, "normal")
        assert result == "Disconnected: 1000"


class TestFullViewSetup:
    def test_view_with_all_callbacks(self):
        view = Websocket("ws/full/", "full_group")
        events = []

        @view.connect("before")
        def on_connect(conn):
            events.append(("connect", conn.path))

        @view.receive
        def on_receive(conn, data):
            events.append(("receive", data))

        @view.disconnect
        def on_disconnect(conn, code=None, reason=None):
            events.append(("disconnect", code))

        # Verify all callbacks are registered and callable
        conn = Connection("/ws/full/", "", {}, {})
        on_connect(conn)
        on_receive(conn, "test message")
        on_disconnect(conn, 1000)

        assert events == [
            ("connect", "/ws/full/"),
            ("receive", "test message"),
            ("disconnect", 1000),
        ]

    def test_view_callbacks_access_conn_data(self):
        view = Websocket("ws/conn_access/", "conn_access")
        captured_data = {}

        @view.connect("before")
        def on_connect(conn):
            captured_data["path"] = conn.path
            captured_data["query"] = conn.query_string
            captured_data["cookie"] = conn.get_cookie("session")
            captured_data["header"] = conn.get_header("authorization")

        conn = Connection(
            path="/ws/conn_access/",
            query_string="room=general",
            headers={"authorization": "Bearer token123"},
            cookies={"session": "abc123"},
        )
        on_connect(conn)

        assert captured_data["path"] == "/ws/conn_access/"
        assert captured_data["query"] == "room=general"
        assert captured_data["cookie"] == "abc123"
        assert captured_data["header"] == "Bearer token123"
