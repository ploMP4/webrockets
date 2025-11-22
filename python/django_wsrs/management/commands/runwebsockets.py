import django_wsrs

from django.utils.module_loading import autodiscover_modules
from django.core.management.base import BaseCommand


class Command(BaseCommand):
    def handle(self, *args, **options):
        autodiscover_modules("websockets", "sockets", "sse", "views")
        django_wsrs.run_server()
