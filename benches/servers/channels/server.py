"""Django Channels benchmark server with echo, db, and compute routes."""

import hashlib
import json
import os
import sqlite3
import sys

os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings")
sys.path.insert(0, os.path.dirname(__file__))

import django
from channels.generic.websocket import WebsocketConsumer
from channels.routing import ProtocolTypeRouter, URLRouter
from django.urls import path

django.setup()


DB_PATH = "/tmp/bench_channels.db"


def init_db():
    """Initialize SQLite database."""
    conn = sqlite3.connect(DB_PATH)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    """)
    conn.commit()
    conn.close()


init_db()


class EchoConsumer(WebsocketConsumer):
    """Simple echo consumer."""

    def connect(self):
        self.accept()

    def disconnect(self, close_code):
        pass

    def receive(self, text_data=None, bytes_data=None):
        if bytes_data:
            self.send(bytes_data=bytes_data)
        elif text_data:
            self.send(text_data=text_data)


class DbConsumer(WebsocketConsumer):
    """Database operations consumer."""

    def connect(self):
        self.accept()

    def disconnect(self, close_code):
        pass

    def receive(self, text_data=None, bytes_data=None):
        data = bytes_data or text_data
        content = data.decode() if isinstance(data, bytes) else data

        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        cursor.execute("INSERT INTO messages (content) VALUES (?)", (content[:100],))
        conn.commit()
        cursor.execute("SELECT id, content FROM messages ORDER BY id DESC LIMIT 10")
        rows = cursor.fetchall()
        conn.close()

        result = json.dumps([{"id": r[0], "content": r[1]} for r in rows])
        self.send(text_data=result)


class ComputeConsumer(WebsocketConsumer):
    """CPU-intensive operations consumer."""

    def connect(self):
        self.accept()

    def disconnect(self, close_code):
        pass

    def receive(self, text_data=None, bytes_data=None):
        data = bytes_data or text_data
        content = data if isinstance(data, bytes) else data.encode()

        result = content
        for _ in range(1000):
            result = hashlib.sha256(result).digest()

        self.send(text_data=result.hex())


application = ProtocolTypeRouter(
    {
        "websocket": URLRouter(
            [
                path("echo", EchoConsumer.as_asgi()),
                path("db", DbConsumer.as_asgi()),
                path("compute", ComputeConsumer.as_asgi()),
            ]
        ),
    }
)

if __name__ == "__main__":
    from daphne.cli import CommandLineInterface

    sys.argv = ["daphne", "-b", "0.0.0.0", "-p", "8000", "server:application"]
    CommandLineInterface.entrypoint()
