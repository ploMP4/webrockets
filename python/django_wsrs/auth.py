from abc import ABC, abstractmethod
from types import SimpleNamespace
from typing import Any
from django.contrib import auth
from django.conf import settings
from django.utils.module_loading import import_string


class AuthenticationFailed(Exception):
    """
    Exception raised when authentication fails.
    """

    def __init__(self, detail: str = "Authentication failed", close_code: int = 4003):
        self.detail = detail
        self.close_code = close_code
        super().__init__(detail)


class BaseAuthentication(ABC):
    """
    Base class for WebSocket authentication.

    All authentication classes should extend this class and implement
    the `authenticate` method.

    Example usage:
        class TokenAuthentication(BaseAuthentication):
            def authenticate(self, scope: ConnectionScope) -> Any:
                token = scope.get_header("Authorization")
                if not token:
                    raise AuthenticationFailed("No token provided")
                user = validate_token(token)
                return user
    """

    @abstractmethod
    def authenticate(self, scope) -> Any | None:
        """
        Authenticate the WebSocket connection request.

        Args:
            scope: ConnectionScope containing headers, cookies, path, and query string.

        Returns:
            A user if authentication succeeds.
            Return None to skip this authenticator and try the next one.

        Raises:
            AuthenticationFailed: If authentication explicitly fails.
        """
        pass


class SessionAuthentication(BaseAuthentication):
    def __init__(self, session_cookie_name: str | None = None):
        """
        Initialize SessionAuthentication.

        Args:
            session_cookie_name: Name of the session cookie. If None, uses
                                 Django's SESSION_COOKIE_NAME setting.
        """
        self._session_cookie_name = session_cookie_name

    @property
    def session_cookie_name(self) -> str:
        if self._session_cookie_name:
            return self._session_cookie_name
        return getattr(settings, "SESSION_COOKIE_NAME", "sessionid")

    def _get_session_store(self):
        engine = getattr(
            settings, "SESSION_ENGINE", "django.contrib.sessions.backends.db"
        )
        return import_string(f"{engine}.SessionStore")

    def authenticate(self, scope) -> Any | None:
        session_id = scope.get_cookie(self.session_cookie_name)
        if not session_id:
            raise AuthenticationFailed("No session cookie found", close_code=4001)

        SessionStore = self._get_session_store()
        session = SessionStore(session_key=session_id)

        request = SimpleNamespace(session=session)
        user = auth.get_user(request)
        if not user.is_authenticated:
            raise AuthenticationFailed(
                "No authenticated user in session", close_code=4001
            )

        return user


class CookieTokenAuthentication(BaseAuthentication):
    """
    Authentication class that reads a token from a cookie.

    This is useful for JWT tokens stored in HTTP-only cookies.
    Users must implement the `validate_token` method or provide a validator function.

    Example:
        class JWTCookieAuth(CookieTokenAuthentication):
            cookie_name = "jwt_token"

            def validate_token(self, token: str) -> Any:
                payload = jwt.decode(token, settings.SECRET_KEY, algorithms=["HS256"])
                return User.objects.get(pk=payload["user_id"])
    """

    cookie_name: str = "auth_token"

    def authenticate(self, scope) -> Any | None:
        token = scope.get_cookie(self.cookie_name)
        if not token:
            raise AuthenticationFailed(
                f"No {self.cookie_name} cookie found", close_code=4001
            )

        user = self.validate_token(token)
        return user

    def validate_token(self, token: str) -> Any:
        """
        Validate the token and return the user.

        Override this method in subclasses to implement token validation.

        Args:
            token: The token string from the cookie.

        Returns:
            The authenticated user object.

        Raises:
            AuthenticationFailed: If the token is invalid.
        """
        raise NotImplementedError("Subclasses must implement validate_token()")


class HeaderTokenAuthentication(BaseAuthentication):
    """
    Authentication class that reads a token from an HTTP header.

    Note: WebSocket connections from browsers cannot set custom headers
    in the initial handshake. This is primarily useful for non-browser clients
    or when using query parameters as a fallback.

    For browser-based JWT authentication, consider:
    - Using CookieTokenAuthentication with HTTP-only cookies
    - Passing tokens via query string (QueryStringTokenAuthentication)
    - Performing authentication after connection via message protocol

    Example for non-browser clients:
        class BearerTokenAuth(HeaderTokenAuthentication):
            header_name = "authorization"
            keyword = "Bearer"

            def validate_token(self, token: str) -> Any:
                return verify_and_get_user(token)
    """

    header_name: str = "authorization"
    keyword: str = "Bearer"

    def authenticate(self, scope) -> Any | None:
        auth_header = scope.get_header(self.header_name)
        if not auth_header:
            raise AuthenticationFailed(
                f"No {self.header_name} header found", close_code=4001
            )

        parts = auth_header.split()
        if len(parts) != 2 or parts[0].lower() != self.keyword.lower():
            raise AuthenticationFailed(
                f"Invalid {self.header_name} header format", close_code=4001
            )

        token = parts[1]
        user = self.validate_token(token)
        return user

    def validate_token(self, token: str) -> Any:
        """
        Validate the token and return the user.

        Override this method in subclasses to implement token validation.
        """
        raise NotImplementedError("Subclasses must implement validate_token()")


class QueryStringTokenAuthentication(BaseAuthentication):
    """
    Authentication class that reads a token from the query string.

    This is useful for browser-based WebSocket connections where custom headers
    cannot be set. The token is passed as a URL parameter.

    Example URL: ws://example.com/chat/?token=your-jwt-token

    Note: Query string tokens may be logged in server access logs.
    Consider using short-lived tokens for this authentication method.

    Example:
        class JWTQueryAuth(QueryStringTokenAuthentication):
            query_param = "token"

            def validate_token(self, token: str) -> Any:
                payload = jwt.decode(token, settings.SECRET_KEY, algorithms=["HS256"])
                return User.objects.get(pk=payload["user_id"])
    """

    query_param: str = "token"

    def authenticate(self, scope) -> Any | None:
        from urllib.parse import parse_qs

        params = parse_qs(scope.query_string)
        tokens = params.get(self.query_param, [])

        if not tokens:
            raise AuthenticationFailed(
                f"No {self.query_param} in query string", close_code=4001
            )

        token = tokens[0]
        user = self.validate_token(token)
        return user

    def validate_token(self, token: str) -> Any:
        """
        Validate the token and return the user.

        Override this method in subclasses to implement token validation.
        """
        raise NotImplementedError("Subclasses must implement validate_token()")
