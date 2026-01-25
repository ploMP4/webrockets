from threading import Thread
from time import sleep

from django.test import TestCase
from django.utils.module_loading import autodiscover_modules

from webrockets.django import server


class WebsocketTestCase(TestCase):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        autodiscover_modules("websockets", "sockets", "views")

    def setUp(self):
        super().setUp()
        self._websocket_thread = Thread(target=server.start)
        self._websocket_thread.start()
        sleep(0.1)

    def tearDown(self):
        super().tearDown()
        server.stop()
        self._websocket_thread.join(timeout=5)
        if self._websocket_thread.is_alive():
            raise RuntimeError("WebSocket server did not shut down cleanly")
