from django.apps import AppConfig


class WsRsAppConfig(AppConfig):
    name = "django_wsrs"
    verbose_name = "django_wsrs"

    def ready(self) -> None:
        pass
