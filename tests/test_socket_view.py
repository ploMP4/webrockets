from django_wsrs import ConnectionScope
from django_wsrs.auth import BaseAuthentication
from django_wsrs.django_wsrs import Websocket


class TestSocketViewCreation:
    def test_create_view_basic(self):
        view = Websocket("ws/test/", "test_group")

        assert view.path == "ws/test/"
        assert view.group == "test_group"

    def test_create_view_with_auth_classes(self):
        class MockAuth(BaseAuthentication):
            def authenticate(self, scope):
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
    def test_connect_decorator_registers_callback(self):
        view = Websocket("ws/connect_test/", "connect_group")
        callback_called = []

        @view.connect
        def on_connect(scope):
            callback_called.append(scope)

        assert callable(on_connect)

    def test_connect_decorator_preserves_function(self):
        view = Websocket("ws/connect_preserve/", "connect_preserve")

        @view.connect
        def my_connect_handler(scope):
            return "connected"

        scope = ConnectionScope("/ws/", "", {}, {})
        result = my_connect_handler(scope)
        assert result == "connected"


class TestReceiveDecorator:
    def test_receive_decorator_registers_callback(self):
        view = Websocket("ws/receive_test/", "receive_group")

        @view.receive
        def on_receive(scope, cid, data):
            pass

        assert callable(on_receive)

    def test_receive_decorator_preserves_function(self):
        view = Websocket("ws/receive_preserve/", "receive_preserve")

        @view.receive
        def my_receive_handler(scope, cid, data):
            return f"Received: {data}"

        scope = ConnectionScope("/ws/", "", {}, {})
        result = my_receive_handler(scope, 123, "hello")
        assert result == "Received: hello"


class TestDisconnectDecorator:
    def test_disconnect_decorator_registers_callback(self):
        view = Websocket("ws/disconnect_test/", "disconnect_group")

        @view.disconnect
        def on_disconnect(scope, code=None, reason=None):
            pass

        assert callable(on_disconnect)

    def test_disconnect_decorator_preserves_function(self):
        view = Websocket("ws/disconnect_preserve/", "disconnect_preserve")

        @view.disconnect
        def my_disconnect_handler(scope, code=None, reason=None):
            return f"Disconnected: {code}"

        scope = ConnectionScope("/ws/", "", {}, {})
        result = my_disconnect_handler(scope, 1000, "normal")
        assert result == "Disconnected: 1000"


class TestFullViewSetup:
    def test_view_with_all_callbacks(self):
        view = Websocket("ws/full/", "full_group")
        events = []

        @view.connect
        def on_connect(scope):
            events.append(("connect", scope.path))

        @view.receive
        def on_receive(scope, cid, data):
            events.append(("receive", data))

        @view.disconnect
        def on_disconnect(scope, code=None, reason=None):
            events.append(("disconnect", code))

        # Verify all callbacks are registered and callable
        scope = ConnectionScope("/ws/full/", "", {}, {})
        on_connect(scope)
        on_receive(scope, 1, "test message")
        on_disconnect(scope, 1000)

        assert events == [
            ("connect", "/ws/full/"),
            ("receive", "test message"),
            ("disconnect", 1000),
        ]

    def test_view_callbacks_access_scope_data(self):
        view = Websocket("ws/scope_access/", "scope_access")
        captured_data = {}

        @view.connect
        def on_connect(scope):
            captured_data["path"] = scope.path
            captured_data["query"] = scope.query_string
            captured_data["cookie"] = scope.get_cookie("session")
            captured_data["header"] = scope.get_header("authorization")

        scope = ConnectionScope(
            path="/ws/scope_access/",
            query_string="room=general",
            headers={"authorization": "Bearer token123"},
            cookies={"session": "abc123"},
        )
        on_connect(scope)

        assert captured_data["path"] == "/ws/scope_access/"
        assert captured_data["query"] == "room=general"
        assert captured_data["cookie"] == "abc123"
        assert captured_data["header"] == "Bearer token123"
