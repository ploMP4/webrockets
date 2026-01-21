# ruff: noqa: E402, I001
# Import order matters: webrockets must be imported first to avoid circular imports
from typing import Literal, TypedDict
from .webrockets import *
from .utils import noop
from .auth import BaseAuthentication


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

__all__ = ["noop", "BaseAuthentication", "BrokerConfig"]

__doc__ = webrockets.__doc__
if hasattr(webrockets, "__all__"):
    __all__ += webrockets.__all__
