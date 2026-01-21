"""Django-Bolt benchmark server with echo, db, and compute routes."""

import hashlib
import json
import os
import sqlite3
import sys

os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings")
sys.path.insert(0, os.path.dirname(__file__))

import django
from django_bolt import BoltAPI, WebSocket, WebSocketDisconnect

django.setup()

api = BoltAPI()

DB_PATH = "/tmp/bench_bolt.db"


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


@api.websocket("/echo")
async def echo_endpoint(websocket: WebSocket):
    """Simple echo endpoint."""
    await websocket.accept()
    try:
        while True:
            data = await websocket.receive_bytes()
            await websocket.send_bytes(data)
    except WebSocketDisconnect:
        pass


@api.websocket("/db")
async def db_endpoint(websocket: WebSocket):
    """Database operations endpoint."""
    await websocket.accept()
    try:
        while True:
            data = await websocket.receive_bytes()
            content = data.decode()

            conn = sqlite3.connect(DB_PATH)
            cursor = conn.cursor()
            cursor.execute("INSERT INTO messages (content) VALUES (?)", (content[:100],))
            conn.commit()
            cursor.execute("SELECT id, content FROM messages ORDER BY id DESC LIMIT 10")
            rows = cursor.fetchall()
            conn.close()

            result = json.dumps([{"id": r[0], "content": r[1]} for r in rows])
            await websocket.send_text(result)
    except WebSocketDisconnect:
        pass


@api.websocket("/compute")
async def compute_endpoint(websocket: WebSocket):
    """CPU-intensive operations endpoint."""
    await websocket.accept()
    try:
        while True:
            data = await websocket.receive_bytes()

            result = data
            for _ in range(1000):
                result = hashlib.sha256(result).digest()

            await websocket.send_text(result.hex())
    except WebSocketDisconnect:
        pass


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(api, host="0.0.0.0", port=1235)
