from .django_wsrs import *
from . import views

__doc__ = django_wsrs.__doc__
if hasattr(django_wsrs, "__all__"):
    __all__ = django_wsrs.__all__
