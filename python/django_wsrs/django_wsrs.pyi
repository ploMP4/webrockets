from typing import Any, Callable, Literal

from django_wsrs.auth import BaseAuthentication

class ConnectionScope:
    path: str
    headers: dict[str, str]
    cookies: dict[str, str]
    query_string: str
    user: Any | None

    def __init__(
        self,
        path: str,
        headers: dict[str, str],
        cookies: dict[str, str],
        query_string: str,
    ) -> None: ...
    def get_cookie(self, name: str) -> str | None: ...
    def get_header(self, name: str) -> str | None: ...

class SocketView:
    path: str
    group: str

    def connect(
        self, func: Callable[[ConnectionScope], None]
    ) -> Callable[[ConnectionScope], None]: ...
    def receive(
        self, func: Callable[[ConnectionScope, int, str], None]
    ) -> Callable[[ConnectionScope, int, str], None]: ...
    def disconnect(
        self, func: Callable[[ConnectionScope], tuple[int, str] | None]
    ) -> Callable[[ConnectionScope, tuple[int, str] | None], None]: ...

class WebsocketServer:
    def __call__(
        self,
        path: str,
        group: str,
        authentication_classes: list[BaseAuthentication] = ...,
    ) -> SocketView: ...
    def start(self) -> None: ...
    def send(self, channel_id: int, msg: str) -> None: ...
    def broadcast_text(self, groups: list[str], msg: str) -> None: ...

Websocket: WebsocketServer

LogLevel = Literal["debug", "info", "warn", "error"]

def log(level: LogLevel, msg: str) -> None: ...
