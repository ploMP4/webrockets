from threading import Thread
from time import sleep

from webrockets import WebsocketServer


class runserver:
    def __init__(self, server: WebsocketServer) -> None:
        self._server = server

    def __enter__(self):
        self._thread = Thread(target=self._server.start)
        self._thread.start()
        sleep(0.1)
        return self

    def __exit__(self, exc_type, exc, tb):
        self._server.stop()
        self._thread.join(timeout=5)
        if self._thread.is_alive():
            raise RuntimeError("WebSocket server did not shut down cleanly")
