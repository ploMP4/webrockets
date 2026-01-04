from abc import ABC, abstractmethod
from typing import Any
from pywsrs import IncomingConnection


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
            def authenticate(self, conn: IncommingConnection) -> Any:
                token = conn.get_header("Authorization")
                if not token:
                    raise AuthenticationFailed("No token provided")
                user = validate_token(token)
                return user
    """

    @abstractmethod
    def authenticate(self, conn: IncomingConnection) -> Any | None:
        """
        Authenticate the WebSocket connection request.

        Args:
            conn: IncommingConnection containing headers, cookies, path, and query string.

        Returns:
            A user if authentication succeeds.
            Return None to skip this authenticator and try the next one.

        Raises:
            AuthenticationFailed: If authentication explicitly fails.
        """
        pass
