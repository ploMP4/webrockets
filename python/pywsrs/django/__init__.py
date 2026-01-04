try:
    import django
except ImportError as e:
    raise ImportError(
        "Django is required for pywsrs.django. "
        'Install with: pip install "pywsrs[django]"'
    ) from e

from .auth import (
    AuthenticationFailed,
    SessionAuthentication,
    CookieTokenAuthentication,
    HeaderTokenAuthentication,
    QueryStringTokenAuthentication,
)

__all__ = [
    "AuthenticationFailed",
    "SessionAuthentication",
    "CookieTokenAuthentication",
    "HeaderTokenAuthentication",
    "QueryStringTokenAuthentication",
]
