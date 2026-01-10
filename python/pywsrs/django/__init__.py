import importlib.util

from pywsrs import WebsocketServer

if importlib.util.find_spec("django") is None:
    raise ImportError(
        'Django is required for pywsrs.django. Install with: pip install "pywsrs[django]"'
    )

from django.conf import settings

from .auth import (
    AuthenticationFailed,
    CookieTokenAuthentication,
    HeaderTokenAuthentication,
    QueryStringTokenAuthentication,
    SessionAuthentication,
)

host = getattr(settings, "WEBSOCKET_HOST", "0.0.0.0")
port = getattr(settings, "WEBSOCKET_PORT", 46290)
broker = getattr(settings, "WEBSOCKET_BROKER", None)
server = WebsocketServer(host, port, broker)

__all__ = [
    "AuthenticationFailed",
    "SessionAuthentication",
    "CookieTokenAuthentication",
    "HeaderTokenAuthentication",
    "QueryStringTokenAuthentication",
    "server",
]
