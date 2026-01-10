import importlib

if importlib.util.find_spec("django") is None:
    raise ImportError(
        'Django is required to use runwebsockets command. Install with: pip install "pywsrs[django]"'
    )

from django.core.management.base import BaseCommand
from django.utils.module_loading import autodiscover_modules

from pywsrs.django import server


class Command(BaseCommand):
    help = "Start the pywsrs WebSocket server"

    def handle(self, *args, **options):
        autodiscover_modules("websockets", "sockets", "sse", "views")
        server.start()
