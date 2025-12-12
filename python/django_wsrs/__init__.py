from .django_wsrs import *
from .auth import (
    AuthenticationFailed,
    BaseAuthentication,
    SessionAuthentication,
    CookieTokenAuthentication,
    HeaderTokenAuthentication,
    QueryStringTokenAuthentication,
)

__doc__ = django_wsrs.__doc__
if hasattr(django_wsrs, "__all__"):
    __all__ = django_wsrs.__all__
