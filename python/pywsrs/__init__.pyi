from typing import (
    TYPE_CHECKING,
    Any,
    Callable,
    Coroutine,
    Generic,
    Literal,
    TypeVar,
    overload,
)

from pywsrs.auth import BaseAuthentication

if TYPE_CHECKING:
    from pydantic import BaseModel

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
T_Schema = TypeVar("T_Schema", bound="BaseModel" | str)

class ConnectDecorator(Generic[T_Connection]):
    def __call__(
        self, func: Callable[[T_Connection], None | Coroutine[Any, Any, None]]
    ) -> Callable[[T_Connection], None | Coroutine[Any, Any, None]]: ...

class ReceiveDecorator(Generic[T_Schema]):
    def __call__(
        self, func: Callable[[Connection, T_Schema], None | Coroutine[Any, Any, None]]
    ) -> Callable[[Connection, T_Schema], None | Coroutine[Any, Any, None]]: ...

class SocketView:
    path: str
    group: str
    discriminator: str

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
    @overload
    def receive_match(self, match: str, /) -> ReceiveDecorator[str]: ...
    @overload
    def receive_match(
        self, match: str, /, schema: type[T_Schema]
    ) -> ReceiveDecorator[T_Schema]: ...
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
        authentication_classes: list[BaseAuthentication] | None = None,
        discriminator: str = "type",
    ) -> SocketView: ...
    def start(self, host: str = "0.0.0.0", port: int = 46290) -> None: ...
    def stop(self) -> None: ...
    def broadcast_text(self, groups: list[str], msg: str) -> None: ...

Websocket: WebsocketServer
