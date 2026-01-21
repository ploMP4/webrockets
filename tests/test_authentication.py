import pytest
from webrockets.auth import AuthenticationFailed
from webrockets.django.auth import (
    CookieTokenAuthentication,
    HeaderTokenAuthentication,
    QueryStringTokenAuthentication,
    SessionAuthentication,
)


@pytest.mark.django_db
class TestSessionAuthentication:
    """Tests for SessionAuthentication."""

    def test_no_session_cookie(self, websocket_scope):
        auth = SessionAuthentication()
        scope = websocket_scope(cookies={})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No session cookie found" in exc_info.value.detail
        assert exc_info.value.close_code == 4001

    def test_invalid_session_id(self, websocket_scope):
        auth = SessionAuthentication()
        scope = websocket_scope(cookies={"sessionid": "invalid-session-id-12345"})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert exc_info.value.close_code == 4001

    def test_valid_session(self, websocket_scope, create_session, active_user):
        session = create_session(user=active_user)
        auth = SessionAuthentication()
        scope = websocket_scope(cookies={"sessionid": session.session_key})

        result = auth.authenticate(scope)

        assert result is not None
        assert result.pk == active_user.pk

    def test_inactive_user_rejected_by_default(self, websocket_scope, session_store, inactive_user):
        session = session_store()
        session["_auth_user_id"] = str(inactive_user.pk)
        session["_auth_user_backend"] = "django.contrib.auth.backends.ModelBackend"
        session.create()

        auth = SessionAuthentication()
        scope = websocket_scope(cookies={"sessionid": session.session_key})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No authenticated user in session" in exc_info.value.detail

    def test_custom_session_cookie_name(self, websocket_scope, create_session, active_user):
        session = create_session(user=active_user)
        auth = SessionAuthentication(session_cookie_name="my_session")

        scope = websocket_scope(cookies={"sessionid": session.session_key})
        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)
        assert "No session cookie found" in exc_info.value.detail

        scope = websocket_scope(cookies={"my_session": session.session_key})
        result = auth.authenticate(scope)
        assert result is not None
        assert result.pk == active_user.pk

    def test_session_without_user_id(self, websocket_scope, session_store):
        session = session_store()
        session["some_data"] = "value"
        session.create()

        auth = SessionAuthentication()
        scope = websocket_scope(cookies={"sessionid": session.session_key})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No authenticated user in session" in exc_info.value.detail

    def test_user_not_found(self, websocket_scope, session_store):
        session = session_store()
        session["_auth_user_id"] = "99999"
        session["_auth_user_backend"] = "django.contrib.auth.backends.ModelBackend"
        session.create()

        auth = SessionAuthentication()
        scope = websocket_scope(cookies={"sessionid": session.session_key})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No authenticated user in session" in exc_info.value.detail

    def test_session_auth_hash_validation(self, websocket_scope, session_store, active_user):
        # Create session with wrong auth hash (simulates password change)
        session = session_store()
        session["_auth_user_id"] = str(active_user.pk)
        session["_auth_user_backend"] = "django.contrib.auth.backends.ModelBackend"
        session["_auth_user_hash"] = "wrong-hash-after-password-change"
        session.create()

        auth = SessionAuthentication()
        scope = websocket_scope(cookies={"sessionid": session.session_key})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No authenticated user in session" in exc_info.value.detail


class TestCookieTokenAuthentication:
    """Tests for CookieTokenAuthentication."""

    def test_no_cookie(self, websocket_scope):
        class TestAuth(CookieTokenAuthentication):
            cookie_name = "auth_token"

            def validate_token(self, token):
                return "user"

        auth = TestAuth()
        scope = websocket_scope(cookies={})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No auth_token cookie found" in exc_info.value.detail

    def test_valid_token(self, websocket_scope):
        class TestAuth(CookieTokenAuthentication):
            cookie_name = "jwt"

            def validate_token(self, token):
                if token == "valid-token":
                    return {"id": 1, "username": "testuser"}
                raise AuthenticationFailed("Invalid token")

        auth = TestAuth()
        scope = websocket_scope(cookies={"jwt": "valid-token"})

        result = auth.authenticate(scope)

        assert result is not None
        assert result == {"id": 1, "username": "testuser"}

    def test_invalid_token(self, websocket_scope):
        class TestAuth(CookieTokenAuthentication):
            cookie_name = "jwt"

            def validate_token(self, token):
                raise AuthenticationFailed("Token expired", close_code=4001)

        auth = TestAuth()
        scope = websocket_scope(cookies={"jwt": "expired-token"})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "Token expired" in exc_info.value.detail

    def test_validate_token_not_implemented(self, websocket_scope):
        auth = CookieTokenAuthentication()
        scope = websocket_scope(cookies={"auth_token": "some-token"})

        with pytest.raises(NotImplementedError):
            auth.authenticate(scope)


class TestHeaderTokenAuthentication:
    """Tests for HeaderTokenAuthentication."""

    def test_no_header(self, websocket_scope):
        class TestAuth(HeaderTokenAuthentication):
            def validate_token(self, token):
                return "user"

        auth = TestAuth()
        scope = websocket_scope(headers={})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No authorization header found" in exc_info.value.detail

    def test_invalid_header_format_no_keyword(self, websocket_scope):
        class TestAuth(HeaderTokenAuthentication):
            def validate_token(self, token):
                return "user"

        auth = TestAuth()
        scope = websocket_scope(headers={"authorization": "token123"})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "Invalid authorization header format" in exc_info.value.detail

    def test_invalid_header_format_wrong_keyword(self, websocket_scope):
        class TestAuth(HeaderTokenAuthentication):
            keyword = "Bearer"

            def validate_token(self, token):
                return "user"

        auth = TestAuth()
        scope = websocket_scope(headers={"authorization": "Basic token123"})

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "Invalid authorization header format" in exc_info.value.detail

    def test_valid_bearer_token(self, websocket_scope):
        class TestAuth(HeaderTokenAuthentication):
            def validate_token(self, token):
                return {"user_id": 1}

        auth = TestAuth()
        scope = websocket_scope(headers={"authorization": "Bearer my-jwt-token"})

        result = auth.authenticate(scope)

        assert result is not None
        assert result == {"user_id": 1}

    def test_custom_header_and_keyword(self, websocket_scope):
        class TestAuth(HeaderTokenAuthentication):
            header_name = "x-api-key"
            keyword = "Key"

            def validate_token(self, token):
                return {"api_key": token}

        auth = TestAuth()
        scope = websocket_scope(headers={"x-api-key": "Key abc123"})

        result = auth.authenticate(scope)

        assert result is not None
        assert result == {"api_key": "abc123"}


class TestQueryStringTokenAuthentication:
    """Tests for QueryStringTokenAuthentication."""

    def test_no_token_in_query(self, websocket_scope):
        class TestAuth(QueryStringTokenAuthentication):
            def validate_token(self, token):
                return "user"

        auth = TestAuth()
        scope = websocket_scope(query_string="room=general")

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No token in query string" in exc_info.value.detail

    def test_empty_query_string(self, websocket_scope):
        class TestAuth(QueryStringTokenAuthentication):
            def validate_token(self, token):
                return "user"

        auth = TestAuth()
        scope = websocket_scope(query_string="")

        with pytest.raises(AuthenticationFailed) as exc_info:
            auth.authenticate(scope)

        assert "No token in query string" in exc_info.value.detail

    def test_valid_token_in_query(self, websocket_scope):
        class TestAuth(QueryStringTokenAuthentication):
            def validate_token(self, token):
                if token == "valid-jwt":
                    return {"user_id": 42}
                raise AuthenticationFailed("Invalid")

        auth = TestAuth()
        scope = websocket_scope(query_string="token=valid-jwt&room=chat")

        result = auth.authenticate(scope)

        assert result is not None
        assert result == {"user_id": 42}

    def test_custom_query_param(self, websocket_scope):
        class TestAuth(QueryStringTokenAuthentication):
            query_param = "access_token"

            def validate_token(self, token):
                return {"token": token}

        auth = TestAuth()
        scope = websocket_scope(query_string="access_token=abc123")

        result = auth.authenticate(scope)

        assert result is not None
        assert result == {"token": "abc123"}

    def test_url_encoded_token(self, websocket_scope):
        class TestAuth(QueryStringTokenAuthentication):
            def validate_token(self, token):
                return token

        auth = TestAuth()
        scope = websocket_scope(query_string="token=hello%20world")

        result = auth.authenticate(scope)

        assert result is not None
        assert result == "hello world"
