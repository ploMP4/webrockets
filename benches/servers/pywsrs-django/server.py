"""pywsrs with Django ORM benchmark server."""

import hashlib
import json
import os
import sys

# Django setup must happen before importing models
os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings")
sys.path.insert(0, os.path.dirname(__file__))

import django

django.setup()

# Create the table if it doesn't exist
from django.db import connection

with connection.cursor() as cursor:
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS bench_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    """)

from models import Message
from pywsrs import Connection
from pywsrs.django import server


def db_operation(data: str | bytes) -> str:
    """Insert a message and retrieve the last 10 using Django ORM."""
    content = data.decode() if isinstance(data, bytes) else data
    Message.objects.create(content=content[:100])
    messages = Message.objects.order_by("-id")[:10]
    return json.dumps([{"id": m.id, "content": m.content} for m in messages])


def compute_operation(data: str | bytes) -> str:
    """CPU-intensive operation: repeated hashing."""
    content = data if isinstance(data, bytes) else data.encode()
    result = content
    for _ in range(1000):
        result = hashlib.sha256(result).digest()
    return result.hex()


# Echo route
echo = server.create_route(path="echo", default_group="echo")


@echo.receive
def echo_receive(conn: Connection, data: str | bytes):
    conn.send(data)


# Database route (using Django ORM)
db = server.create_route(path="db", default_group="db")


@db.receive
def db_receive(conn: Connection, data: str | bytes):
    result = db_operation(data)
    conn.send(result)


# Compute route
compute = server.create_route(path="compute", default_group="compute")


@compute.receive
def compute_receive(conn: Connection, data: str | bytes):
    result = compute_operation(data)
    conn.send(result)


if __name__ == "__main__":
    server.start()
