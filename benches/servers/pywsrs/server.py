"""pywsrs standalone benchmark server with echo, db, and compute routes."""

import hashlib
import json
import sqlite3
from pathlib import Path

from pywsrs import Connection, WebsocketServer

DB_PATH = Path("/tmp/bench.db")


def init_db():
    """Initialize SQLite database with a simple table."""
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


def db_operation(data: str | bytes) -> str:
    """Insert a message and retrieve the last 10."""
    content = data.decode() if isinstance(data, bytes) else data
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("INSERT INTO messages (content) VALUES (?)", (content[:100],))
    conn.commit()
    cursor.execute("SELECT id, content FROM messages ORDER BY id DESC LIMIT 10")
    rows = cursor.fetchall()
    conn.close()
    return json.dumps([{"id": r[0], "content": r[1]} for r in rows])


def compute_operation(data: str | bytes) -> str:
    """CPU-intensive operation: repeated hashing."""
    content = data if isinstance(data, bytes) else data.encode()
    result = content
    for _ in range(1000):
        result = hashlib.sha256(result).digest()
    return result.hex()


init_db()

server = WebsocketServer(host="0.0.0.0", port=6969)

# Echo route
echo = server.create_route(path="echo", default_group="echo")


@echo.receive
def echo_receive(conn: Connection, data: str | bytes):
    conn.send(data)


# Database route
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
