from abc import ABC, abstractmethod
from typing import Any

from django_wsrs import ConnectionScope

class AuthenticationFailed(Exception):
    detail: str
    close_code: int
    def __init__(
        self, detail: str = "Authentication failed", close_code: int = 4003
    ) -> None: ...

class BaseAuthentication(ABC):
    @abstractmethod
    def authenticate(self, scope: ConnectionScope) -> Any | None: ...

class SessionAuthentication(BaseAuthentication):
    def __init__(self, session_cookie_name: str | None = None) -> None: ...
    @property
    def session_cookie_name(self) -> str: ...
    def authenticate(self, scope: ConnectionScope) -> Any | None: ...

class CookieTokenAuthentication(BaseAuthentication):
    cookie_name: str
    def authenticate(self, scope: ConnectionScope) -> Any | None: ...
    def validate_token(self, token: str) -> Any: ...

class HeaderTokenAuthentication(BaseAuthentication):
    header_name: str
    keyword: str
    def authenticate(self, scope: ConnectionScope) -> Any | None: ...
    def validate_token(self, token: str) -> Any: ...

class QueryStringTokenAuthentication(BaseAuthentication):
    query_param: str
    def authenticate(self, scope: ConnectionScope) -> Any | None: ...
    def validate_token(self, token: str) -> Any: ...
