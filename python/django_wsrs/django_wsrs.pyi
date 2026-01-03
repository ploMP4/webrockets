from typing import Any, Callable, Generic, Literal, TypeVar, overload

from django_wsrs.auth import BaseAuthentication

class BaseConnection:
    path: str
    query_string: str
    headers: dict[str, str]
    cookies: dict[str, str]
    user: Any | None

    def __init__(
        self,
        path: str,
        query_string: str,
        headers: dict[str, str],
        cookies: dict[str, str],
    ) -> None: ...
    def get_cookie(self, name: str) -> str | None: ...
    def get_header(self, name: str) -> str | None: ...

class IncomingConnection(BaseConnection): ...

class Connection(BaseConnection):
    def send(self, msg: str) -> None: ...
    async def asend(self, msg: str) -> None: ...

T_Connection = TypeVar("T_Connection", bound=IncomingConnection | Connection)

class ConnectDecorator(Generic[T_Connection]):
    def __call__(
        self, func: Callable[[T_Connection], None]
    ) -> Callable[[T_Connection], None]: ...

class SocketView:
    path: str
    group: str

    @overload
    def connect(
        self,
        when: Literal["before"],
    ) -> ConnectDecorator[IncomingConnection]: ...
    @overload
    def connect(
        self,
        when: Literal["after"],
    ) -> ConnectDecorator[Connection]: ...
    def receive(
        self, func: Callable[[Connection, str], None]
    ) -> Callable[[Connection, str], None]: ...
    def disconnect(
        self, func: Callable[[Connection, int | None, str | None], None]
    ) -> Callable[[Connection, int | None, str | None], None]: ...

class WebsocketServer:
    def __call__(
        self,
        path: str,
        group: str,
        authentication_classes: list[BaseAuthentication] = ...,
    ) -> SocketView: ...
    def start(self) -> None: ...
    def broadcast_text(self, groups: list[str], msg: str) -> None: ...

Websocket: WebsocketServer
