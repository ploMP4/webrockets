from django_wsrs import ConnectionScope


class TestConnectionScopeCreation:
    def test_create_with_all_parameters(self):
        scope = ConnectionScope(
            path="/ws/chat/",
            query_string="room=general&user=alice",
            headers={
                "authorization": "Bearer token123",
                "content-type": "application/json",
            },
            cookies={"sessionid": "abc123", "csrftoken": "xyz789"},
        )

        assert scope.path == "/ws/chat/"
        assert scope.query_string == "room=general&user=alice"
        assert scope.headers == {
            "authorization": "Bearer token123",
            "content-type": "application/json",
        }
        assert scope.cookies == {"sessionid": "abc123", "csrftoken": "xyz789"}

    def test_create_with_empty_values(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={},
            cookies={},
        )

        assert scope.path == "/ws/"
        assert scope.query_string == ""
        assert scope.headers == {}
        assert scope.cookies == {}

    def test_user_is_none_by_default(self):
        scope = ConnectionScope(
            path="/ws/test/",
            query_string="",
            headers={},
            cookies={},
        )

        assert scope.user is None


class TestConnectionScopeCookies:
    def test_get_cookie_existing(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={},
            cookies={"sessionid": "abc123", "auth_token": "jwt-token"},
        )

        assert scope.get_cookie("sessionid") == "abc123"
        assert scope.get_cookie("auth_token") == "jwt-token"

    def test_get_cookie_nonexistent(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={},
            cookies={"sessionid": "abc123"},
        )

        assert scope.get_cookie("nonexistent") is None

    def test_get_cookie_empty_cookies(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={},
            cookies={},
        )

        assert scope.get_cookie("any_cookie") is None


class TestConnectionScopeHeaders:
    def test_get_header_existing(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={"authorization": "Bearer token", "x-custom-header": "value"},
            cookies={},
        )

        assert scope.get_header("authorization") == "Bearer token"
        assert scope.get_header("x-custom-header") == "value"

    def test_get_header_nonexistent(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={"authorization": "Bearer token"},
            cookies={},
        )

        assert scope.get_header("nonexistent") is None

    def test_get_header_empty_headers(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={},
            cookies={},
        )

        assert scope.get_header("any_header") is None

    def test_headers_are_case_sensitive(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={"authorization": "token1", "Authorization": "token2"},
            cookies={},
        )

        assert scope.get_header("authorization") == "token1"
        assert scope.get_header("Authorization") == "token2"


class TestConnectionScopeSpecialCharacters:
    def test_path_with_special_characters(self):
        scope = ConnectionScope(
            path="/ws/chat/room-123/user_abc/",
            query_string="",
            headers={},
            cookies={},
        )

        assert scope.path == "/ws/chat/room-123/user_abc/"

    def test_query_string_with_encoded_values(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="msg=hello%20world&name=John%20Doe",
            headers={},
            cookies={},
        )

        assert scope.query_string == "msg=hello%20world&name=John%20Doe"

    def test_cookie_with_special_values(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={},
            cookies={
                "jwt": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
            },
        )

        assert scope.get_cookie("jwt").startswith("eyJ")

    def test_header_with_unicode(self):
        scope = ConnectionScope(
            path="/ws/",
            query_string="",
            headers={"x-user-name": "用户名"},
            cookies={},
        )

        assert scope.get_header("x-user-name") == "用户名"
