import pytest
from pywsrs import Connection, Match, Websocket


class TestMatchCreation:
    """Test Match object creation with various inputs."""

    def test_single_key_single_value(self):
        m = Match("type", "message")
        assert m.key == ["type"]
        assert m.value == ["message"]
        assert m.remove_key is False

    def test_single_key_single_value_with_remove_key(self):
        m = Match("type", "message", True)
        assert m.key == ["type"]
        assert m.value == ["message"]
        assert m.remove_key is True

    def test_multiple_keys_single_value(self):
        m = Match(["type", "action"], "message")
        assert m.key == ["type", "action"]
        assert m.value == ["message"]

    def test_single_key_multiple_values(self):
        m = Match("type", ["message", "notification"])
        assert m.key == ["type"]
        assert m.value == ["message", "notification"]

    def test_multiple_keys_multiple_values(self):
        m = Match(["type", "action"], ["message", "notification"])
        assert m.key == ["type", "action"]
        assert m.value == ["message", "notification"]

    def test_tuple_keys(self):
        m = Match(("type", "action"), "message")
        assert m.key == ["type", "action"]

    def test_tuple_values(self):
        m = Match("type", ("message", "notification"))
        assert m.value == ["message", "notification"]

    def test_generator_keys(self):
        m = Match((k for k in ["type", "action"]), "message")
        assert m.key == ["type", "action"]

    def test_generator_values(self):
        m = Match("type", (v for v in ["message", "notification"]))
        assert m.value == ["message", "notification"]


class TestMatchWithIntegers:
    """Test Match with integer values."""

    def test_single_integer_value(self):
        m = Match("code", 1)
        assert m.key == ["code"]
        assert m.value == [1]

    def test_multiple_integer_values(self):
        m = Match("code", [1, 2, 3])
        assert m.key == ["code"]
        assert m.value == [1, 2, 3]

    def test_mixed_string_and_integer_values(self):
        m = Match("type", ["message", 1, "notification", 2])
        assert m.key == ["type"]
        assert m.value == ["message", 1, "notification", 2]


class TestMatchWildcard:
    """Test Match with wildcard values."""

    def test_wildcard_single(self):
        m = Match("type", "*")
        assert m.key == ["type"]
        assert m.value == ["*"]

    def test_wildcard_with_other_values(self):
        m = Match("type", ["message", "*"])
        assert m.key == ["type"]
        assert m.value == ["message", "*"]

    def test_wildcard_multiple_keys(self):
        m = Match(["type", "action"], "*")
        assert m.key == ["type", "action"]
        assert m.value == ["*"]


class TestMatchValidation:
    """Test Match validation errors."""

    def test_empty_keys_raises_error(self):
        with pytest.raises(ValueError, match="must not be empty"):
            Match([], "message")

    def test_empty_values_raises_error(self):
        with pytest.raises(ValueError, match="must not be empty"):
            Match("type", [])

    def test_invalid_key_type_raises_error(self):
        with pytest.raises(TypeError):
            Match(123, "message")  # Key must be string

    def test_invalid_value_type_raises_error(self):
        with pytest.raises(TypeError):
            Match("type", 3.14)  # Floats are not supported, only str/int

    def test_non_string_in_key_iterable_raises_error(self):
        with pytest.raises(TypeError, match="key iterable must contain only strings"):
            Match(["type", 123], "message")


class TestReceiveWithMatch:
    """Test receive decorator with Match objects."""

    def test_receive_with_single_match(self):
        view = Websocket("ws/test/", "test")

        @view.receive(match=Match("type", "message"))
        def on_message(conn, data):
            pass

        assert callable(on_message)

    def test_receive_with_multiple_values_match(self):
        view = Websocket("ws/test2/", "test2")

        @view.receive(match=Match("type", ["message", "notification"]))
        def on_message(conn, data):
            pass

        assert callable(on_message)

    def test_receive_with_multiple_keys_match(self):
        view = Websocket("ws/test3/", "test3")

        @view.receive(match=Match(["type", "action"], "message"))
        def on_message(conn, data):
            pass

        assert callable(on_message)

    def test_receive_with_wildcard_match(self):
        view = Websocket("ws/test4/", "test4")

        @view.receive(match=Match("type", "*"))
        def on_any_type(conn, data):
            pass

        assert callable(on_any_type)

    def test_receive_with_integer_match(self):
        view = Websocket("ws/test5/", "test5")

        @view.receive(match=Match("code", 1))
        def on_code_1(conn, data):
            pass

        assert callable(on_code_1)

    def test_receive_preserves_function(self):
        view = Websocket("ws/test6/", "test6")

        @view.receive(match=Match("type", "echo"))
        def echo_handler(conn, data):
            return f"Echo: {data}"

        conn = Connection("/ws/test6/", "", {}, {})
        result = echo_handler(conn, "hello")
        assert result == "Echo: hello"


class TestReceiveMatchDuplicateRegistration:
    """Test that duplicate match registrations are rejected."""

    def test_duplicate_exact_match_raises_error(self):
        view = Websocket("ws/dup1/", "dup1")

        @view.receive(match=Match("type", "message"))
        def handler1(conn, data):
            pass

        with pytest.raises(ValueError, match="already registered"):

            @view.receive(match=Match("type", "message"))
            def handler2(conn, data):
                pass

    def test_duplicate_in_multiple_values_raises_error(self):
        view = Websocket("ws/dup2/", "dup2")

        @view.receive(match=Match("type", "message"))
        def handler1(conn, data):
            pass

        with pytest.raises(ValueError, match="already registered"):

            @view.receive(match=Match("type", ["notification", "message"]))
            def handler2(conn, data):
                pass

    def test_duplicate_in_multiple_keys_raises_error(self):
        view = Websocket("ws/dup3/", "dup3")

        @view.receive(match=Match("type", "message"))
        def handler1(conn, data):
            pass

        with pytest.raises(ValueError, match="already registered"):

            @view.receive(match=Match(["type", "action"], "message"))
            def handler2(conn, data):
                pass

    def test_different_values_same_key_allowed(self):
        view = Websocket("ws/nodup1/", "nodup1")

        @view.receive(match=Match("type", "message"))
        def handler1(conn, data):
            pass

        @view.receive(match=Match("type", "notification"))
        def handler2(conn, data):
            pass

        # Both should be registered without error
        assert callable(handler1)
        assert callable(handler2)

    def test_different_keys_same_value_allowed(self):
        view = Websocket("ws/nodup2/", "nodup2")

        @view.receive(match=Match("type", "message"))
        def handler1(conn, data):
            pass

        @view.receive(match=Match("action", "message"))
        def handler2(conn, data):
            pass

        # Both should be registered without error
        assert callable(handler1)
        assert callable(handler2)


class TestReceiveWithGenericFallback:
    """Test receive with both match handlers and generic fallback."""

    def test_match_and_generic_handlers(self):
        view = Websocket("ws/fallback/", "fallback")

        @view.receive(match=Match("type", "specific"))
        def specific_handler(conn, data):
            return "specific"

        @view.receive
        def generic_handler(conn, data):
            return "generic"

        assert callable(specific_handler)
        assert callable(generic_handler)

    def test_duplicate_generic_raises_error(self):
        view = Websocket("ws/dupgen/", "dupgen")

        @view.receive
        def handler1(conn, data):
            pass

        with pytest.raises(ValueError, match="already registered"):

            @view.receive
            def handler2(conn, data):
                pass


class TestMatchWithRemoveKey:
    """Test Match with remove_key option."""

    def test_remove_key_false_by_default(self):
        m = Match("type", "message")
        assert m.remove_key is False

    def test_remove_key_true(self):
        m = Match("type", "message", True)
        assert m.remove_key is True

    def test_remove_key_with_multiple_keys(self):
        m = Match(["type", "action"], "message", True)
        assert m.remove_key is True
        assert m.key == ["type", "action"]
