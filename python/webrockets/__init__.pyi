from collections.abc import Callable, Coroutine, Iterable
from typing import (
    TYPE_CHECKING,
    Any,
    Generic,
    Literal,
    TypedDict,
    TypeVar,
    overload,
)

from webrockets.auth import BaseAuthentication

if TYPE_CHECKING:
    from pydantic import BaseModel

# Type aliases for Match parameters
MatchKey = str | Iterable[str]
MatchValue = str | int | Literal["*"] | Iterable[str | int | Literal["*"]]

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
    def broadcast(
        self, groups: list[str], msg: str | bytes, exclude_self: bool = False
    ) -> None: ...
    async def abroadcast(
        self, groups: list[str], msg: str | bytes, exclude_self: bool = False
    ) -> None: ...
    def join(self, group: str) -> bool:
        """Join a group dynamically. Returns True if newly joined, False if already a member."""
        ...
    def leave(self, group: str) -> bool:
        """Leave a group dynamically. Returns True if was a member, False otherwise."""
        ...
    def groups(self) -> list[str]:
        """Get all groups this connection belongs to."""
        ...
    def group_size(self, group: str) -> int:
        """Get the number of connections in a group."""
        ...

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

class Match:
    """
    Pattern matcher for receive handlers.

    Supports matching on JSON message fields with:
    - Single or multiple keys: Match("type", ...) or Match(["type", "action"], ...)
    - Single or multiple values: Match(..., "message") or Match(..., ["msg", "notify"])
    - Integer values: Match("code", 1) or Match("code", [1, 2, 3])
    - Wildcard matching: Match("type", "*") matches any value
    - Mixed types: Match("type", ["message", 1, "*"])

    Args:
        key: Field name(s) to match on (string or iterable of strings)
        value: Value(s) to match (string, int, "*" for wildcard, or iterable)
        remove_key: If True, removes the matched key from the JSON before passing to handler
    """

    key: list[str]
    value: list[str | int]
    remove_key: bool

    def __init__(
        self,
        key: MatchKey,
        value: MatchValue,
        *,
        remove_key: bool = False,
    ) -> None: ...

class WebsocketRoute:
    path: str
    default_group: str | None
    authentication_classes: list[BaseAuthentication] | None = None

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
    @overload
    def receive(
        self,
        func: Callable[[Connection, str | bytes], None | Coroutine[Any, Any, None]],
        /,
    ) -> Callable[[Connection, str | bytes], None | Coroutine[Any, Any, None]]: ...
    @overload
    def receive(
        self,
        /,
        match: Match,
    ) -> ReceiveDecorator[str]: ...
    @overload
    def receive(
        self,
        /,
        match: Match,
        schema: type[T_Schema],
    ) -> ReceiveDecorator[T_Schema]: ...
    def disconnect(
        self,
        func: Callable[[Connection, int | None, str | None], None | Coroutine[Any, Any, None]],
    ) -> Callable[[Connection, int | None, str | None], None | Coroutine[Any, Any, None]]: ...

class WebsocketServer:
    def __init__(
        self,
        host: str = "0.0.0.0",
        port: int = 46290,
        broker: BrokerConfig | None = None,
    ) -> None: ...
    def start(self) -> None: ...
    def stop(self) -> None: ...
    def create_route(
        self,
        path: str,
        default_group: str | None = None,
        authentication_classes: list[BaseAuthentication] | None = None,
    ) -> WebsocketRoute: ...
    def addr(self) -> str: ...

# Broker functions for external broadcasting (e.g., from Django views, Celery tasks)
def setup_broadcast(config: BrokerConfig) -> None:
    """
    Initialize the broadcaster with a broker configuration.
    Must be called before using broadcast() or abroadcast().
    """
    ...

def reset_broadcast() -> None:
    """
    Reset the broadcaster, allowing setup_broadcast() to be called again.
    Primarily useful for testing when switching between broker configurations.
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
