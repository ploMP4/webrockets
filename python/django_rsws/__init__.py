from .django_rsws import *
from . import views

__doc__ = django_rsws.__doc__
if hasattr(django_rsws, "__all__"):
    __all__ = django_rsws.__all__
