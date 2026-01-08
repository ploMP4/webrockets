from django.conf import settings
from django.core.management.base import BaseCommand
from django.utils.module_loading import autodiscover_modules

import pywsrs


class Command(BaseCommand):
    help = "Start the pywsrs WebSocket server"

    def handle(self, *args, **options):
        autodiscover_modules("websockets", "sockets", "sse", "views")

        host = getattr(settings, "WEBSOCKET_HOST", "0.0.0.0")
        port = getattr(settings, "WEBSOCKET_PORT", 46290)
        broker = getattr(settings, "WEBSOCKET_BROKER", None)

        pywsrs.Websocket.start(host, port, broker=broker)
