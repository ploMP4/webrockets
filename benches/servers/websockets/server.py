"""Pure websockets library benchmark server with echo, db, and compute routes."""

import asyncio
import hashlib
import json
import sqlite3

from websockets.asyncio.server import serve

DB_PATH = "/tmp/bench_websockets.db"


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


async def echo_handler(websocket):
    """Simple echo handler."""
    async for message in websocket:
        await websocket.send(message)


async def db_handler(websocket):
    """Database operations handler."""
    async for message in websocket:
        content = message.decode() if isinstance(message, bytes) else message

        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        cursor.execute("INSERT INTO messages (content) VALUES (?)", (content[:100],))
        conn.commit()
        cursor.execute("SELECT id, content FROM messages ORDER BY id DESC LIMIT 10")
        rows = cursor.fetchall()
        conn.close()

        result = json.dumps([{"id": r[0], "content": r[1]} for r in rows])
        await websocket.send(result)


async def compute_handler(websocket):
    """CPU-intensive operations handler."""
    async for message in websocket:
        content = message if isinstance(message, bytes) else message.encode()

        result = content
        for _ in range(1000):
            result = hashlib.sha256(result).digest()

        await websocket.send(result.hex())


async def router(websocket):
    """Route requests based on path."""
    path = websocket.request.path

    if path == "/echo":
        await echo_handler(websocket)
    elif path == "/db":
        await db_handler(websocket)
    elif path == "/compute":
        await compute_handler(websocket)
    else:
        await websocket.close(1008, "Unknown path")


async def main():
    async with serve(router, "0.0.0.0", 4200) as server:
        await server.serve_forever()


if __name__ == "__main__":
    asyncio.run(main())
