import django_rsws

from django.apps import AppConfig


class WsRsAppConfig(AppConfig):
    name = "django_rsws"
    verbose_name = "django_rsws"

    def ready(self) -> None:
        django_rsws.start_server()
