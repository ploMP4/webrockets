import importlib

if importlib.util.find_spec("django") is None:
    raise ImportError(
        'Django is required to use apps.py. Install with: pip install "pywsrs[django]"'
    )

from django.apps import AppConfig


class WsRsAppConfig(AppConfig):
    name = "pywsrs"
    verbose_name = "pywsrs"

    def ready(self) -> None:
        from django.conf import settings

        broker_config = getattr(settings, "WEBSOCKET_BROKER", None)
        if broker_config:
            from pywsrs import setup_broadcast

            setup_broadcast(broker_config)
