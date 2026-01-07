from django.apps import AppConfig


class WsRsAppConfig(AppConfig):
    name = "pywsrs.django"
    verbose_name = "pywsrs"

    def ready(self) -> None:
        from django.conf import settings

        broker_config = getattr(settings, "WEBSOCKET_BROKER", None)
        if broker_config:
            from pywsrs import setup_broadcast

            setup_broadcast(broker_config)
