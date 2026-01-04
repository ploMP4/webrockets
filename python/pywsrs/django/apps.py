from django.apps import AppConfig


class WsRsAppConfig(AppConfig):
    name = "pywsrs.django"
    verbose_name = "pywsrs"

    def ready(self) -> None:
        pass
