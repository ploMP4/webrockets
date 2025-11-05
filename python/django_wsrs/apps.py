import django_wsrs
import os

from django.apps import AppConfig


class WsRsAppConfig(AppConfig):
    name = "django_wsrs"
    verbose_name = "django_wsrs"

    def ready(self) -> None:
        if (
            os.environ.get("RUN_MAIN") == "true"
            or os.environ.get("WERKZEUG_RUN_MAIN") == "true"
        ):
            django_wsrs.start_server()
