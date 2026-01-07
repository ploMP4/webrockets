from typing import (
    TYPE_CHECKING,
    Any,
    Callable,
    Coroutine,
    Generic,
    Literal,
    TypedDict,
    TypeVar,
    overload,
)

from pywsrs.auth import BaseAuthentication

if TYPE_CHECKING:
    from pydantic import BaseModel

class _RedisBrokerConfigOptional(TypedDict, total=False):
    url: str  # default: "redis://localhost:6379"
    channel: str  # default: "ws_broadcast"

class RedisBrokerConfig(_RedisBrokerConfigOptional):
    type: Literal["redis"]

class _AmqpBrokerConfigOptional(TypedDict, total=False):
    url: str  # default: "amqp://localhost:5672"
    exchange: str  # default: "ws_broadcast"
    queue: str | None  # default: auto-generated UUID
    routing_key: str  # default: "#"

class AmqpBrokerConfig(_AmqpBrokerConfigOptional):
    type: Literal["amqp"]

BrokerConfig = RedisBrokerConfig | AmqpBrokerConfig

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
    def start(
        self,
        host: str = "0.0.0.0",
        port: int = 46290,
        broker: BrokerConfig | None = None,
    ) -> None: ...
    def stop(self) -> None: ...
    def broadcast_text(self, groups: list[str], msg: str) -> None: ...

Websocket: WebsocketServer

# Broker functions for external broadcasting (e.g., from Django views, Celery tasks)
def setup_broadcast(config: BrokerConfig) -> None:
    """
    Initialize the broadcaster with a broker configuration.
    Must be called before using broadcast() or abroadcast().
    Can only be called once per process (uses OnceLock).
    """
    ...

def broadcast(groups: list[str], message: str) -> None:
    """
    Publish a message to the specified groups via the configured broker.
    This is a blocking call that publishes synchronously.

    Args:
        groups: List of group names to broadcast to
        message: The message payload (typically JSON string)
    """
    ...

async def abroadcast(groups: list[str], message: str) -> None:
    """
    Async version of broadcast().
    Publish a message to the specified groups via the configured broker.

    Args:
        groups: List of group names to broadcast to
        message: The message payload (typically JSON string)
    """
    ...
