from typing import Any, Callable, Coroutine, Generic, Literal, TypeVar, overload

from pywsrs.auth import BaseAuthentication

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
    def send(self, msg: str | bytes) -> None: ...
    async def asend(self, msg: str | bytes) -> None: ...
    def close(self, code: int = 1000, reason: str = "") -> None: ...
    async def aclose(self, code: int = 1000, reason: str = "") -> None: ...

T_Connection = TypeVar("T_Connection", bound=IncomingConnection | Connection)

class ConnectDecorator(Generic[T_Connection]):
    def __call__(
        self, func: Callable[[T_Connection], None | Coroutine[Any, Any, None]]
    ) -> Callable[[T_Connection], None | Coroutine[Any, Any, None]]: ...

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
        self,
        func: Callable[[Connection, str | bytes], None | Coroutine[Any, Any, None]],
    ) -> Callable[[Connection, str | bytes], None | Coroutine[Any, Any, None]]: ...
    def disconnect(
        self,
        func: Callable[
            [Connection, int | None, str | None], None | Coroutine[Any, Any, None]
        ],
    ) -> Callable[
        [Connection, int | None, str | None], None | Coroutine[Any, Any, None]
    ]: ...

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
