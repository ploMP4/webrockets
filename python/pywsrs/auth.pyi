from abc import ABC, abstractmethod
from typing import Any

from pywsrs import IncomingConnection

class AuthenticationFailed(Exception):
    detail: str
    close_code: int
    def __init__(
        self, detail: str = "Authentication failed", close_code: int = 4003
    ) -> None: ...

class BaseAuthentication(ABC):
    @abstractmethod
    def authenticate(self, conn: IncomingConnection) -> Any | None: ...
