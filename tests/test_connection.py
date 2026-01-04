from pywsrs import Connection


class TestConnectionCreation:
    def test_create_with_all_parameters(self):
        conn = Connection(
            path="/ws/chat/",
            query_string="room=general&user=alice",
            headers={
                "authorization": "Bearer token123",
                "content-type": "application/json",
            },
            cookies={"sessionid": "abc123", "csrftoken": "xyz789"},
        )

        assert conn.path == "/ws/chat/"
        assert conn.query_string == "room=general&user=alice"
        assert conn.headers == {
            "authorization": "Bearer token123",
            "content-type": "application/json",
        }
        assert conn.cookies == {"sessionid": "abc123", "csrftoken": "xyz789"}

    def test_create_with_empty_values(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={},
            cookies={},
        )

        assert conn.path == "/ws/"
        assert conn.query_string == ""
        assert conn.headers == {}
        assert conn.cookies == {}

    def test_user_is_none_by_default(self):
        conn = Connection(
            path="/ws/test/",
            query_string="",
            headers={},
            cookies={},
        )

        assert conn.user is None


class TestConnectionCookies:
    def test_get_cookie_existing(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={},
            cookies={"sessionid": "abc123", "auth_token": "jwt-token"},
        )

        assert conn.get_cookie("sessionid") == "abc123"
        assert conn.get_cookie("auth_token") == "jwt-token"

    def test_get_cookie_nonexistent(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={},
            cookies={"sessionid": "abc123"},
        )

        assert conn.get_cookie("nonexistent") is None

    def test_get_cookie_empty_cookies(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={},
            cookies={},
        )

        assert conn.get_cookie("any_cookie") is None


class TestConnectionHeaders:
    def test_get_header_existing(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={"authorization": "Bearer token", "x-custom-header": "value"},
            cookies={},
        )

        assert conn.get_header("authorization") == "Bearer token"
        assert conn.get_header("x-custom-header") == "value"

    def test_get_header_nonexistent(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={"authorization": "Bearer token"},
            cookies={},
        )

        assert conn.get_header("nonexistent") is None

    def test_get_header_empty_headers(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={},
            cookies={},
        )

        assert conn.get_header("any_header") is None

    def test_headers_are_case_sensitive(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={"authorization": "token1", "Authorization": "token2"},
            cookies={},
        )

        assert conn.get_header("authorization") == "token1"
        assert conn.get_header("Authorization") == "token2"


class TestConnectionSpecialCharacters:
    def test_path_with_special_characters(self):
        conn = Connection(
            path="/ws/chat/room-123/user_abc/",
            query_string="",
            headers={},
            cookies={},
        )

        assert conn.path == "/ws/chat/room-123/user_abc/"

    def test_query_string_with_encoded_values(self):
        conn = Connection(
            path="/ws/",
            query_string="msg=hello%20world&name=John%20Doe",
            headers={},
            cookies={},
        )

        assert conn.query_string == "msg=hello%20world&name=John%20Doe"

    def test_cookie_with_special_values(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={},
            cookies={
                "jwt": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
            },
        )

        assert conn.get_cookie("jwt").startswith("eyJ")

    def test_header_with_unicode(self):
        conn = Connection(
            path="/ws/",
            query_string="",
            headers={"x-user-name": "用户名"},
            cookies={},
        )

        assert conn.get_header("x-user-name") == "用户名"
