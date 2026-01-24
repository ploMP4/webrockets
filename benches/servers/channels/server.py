import os
import sys

os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings")
sys.path.insert(0, os.path.dirname(__file__))

import django
from channels.generic.websocket import WebsocketConsumer
from channels.routing import ProtocolTypeRouter, URLRouter
from django.urls import path

django.setup()


class EchoConsumer(WebsocketConsumer):
    """Simple echo consumer."""

    def connect(self):
        self.accept()

    def disconnect(self, close_code):
        pass

    def receive(self, text_data=None, bytes_data=None):
        self.send(bytes_data=bytes_data)


application = ProtocolTypeRouter(
    {
        "websocket": URLRouter(
            [
                path("echo", EchoConsumer.as_asgi()),
            ]
        ),
    }
)

if __name__ == "__main__":
    from daphne.cli import CommandLineInterface

    sys.argv = ["daphne", "-b", "0.0.0.0", "-p", "8000", "server:application"]
    CommandLineInterface.entrypoint()
