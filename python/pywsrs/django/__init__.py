import importlib.util

if importlib.util.find_spec("django") is None:
    raise ImportError(
        'Django is required for pywsrs.django. Install with: pip install "pywsrs[django]"'
    )

from .auth import (
    AuthenticationFailed,
    CookieTokenAuthentication,
    HeaderTokenAuthentication,
    QueryStringTokenAuthentication,
    SessionAuthentication,
)

__all__ = [
    "AuthenticationFailed",
    "SessionAuthentication",
    "CookieTokenAuthentication",
    "HeaderTokenAuthentication",
    "QueryStringTokenAuthentication",
]
