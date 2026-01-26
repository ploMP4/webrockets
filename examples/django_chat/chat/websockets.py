"""
WebSocket routes for the chat application.

This module is automatically discovered by webrockets when running:
    python manage.py runwebsockets
"""

import json
import logging

from webrockets import Match, broadcast
from webrockets.django import server

logger = logging.getLogger(__name__)


# Create the chat route (no authentication for this example)
chat = server.create_route("ws/chat/")


@chat.connect("after")
def on_connect(conn):
    logger.info(f"Client connected: {conn.path}")
    conn.send(
        json.dumps(
            {
                "type": "system",
                "message": "Connected to chat server. Send a 'join' message to enter a room.",
            }
        )
    )


@chat.receive(match=Match("type", "join"))
def handle_join(conn, msg: str | bytes):
    msg = json.loads(msg)
    room = msg.get("room", "general")
    username = msg.get("username", "Anonymous")

    # Store username on connection
    conn.user = username

    # Leave any existing rooms
    for group in conn.groups():
        conn.leave(group)

    # Join the new room
    conn.join(room)

    logger.info(f"{username} joined room: {room}")

    # Notify the user
    conn.send(
        json.dumps(
            {
                "type": "joined",
                "room": room,
                "message": f"You joined {room}",
            }
        )
    )

    # Broadcast to room that user joined (via Redis for multi-server)
    broadcast(
        [room],
        json.dumps(
            {
                "type": "user_joined",
                "username": username,
                "room": room,
            }
        ),
    )


@chat.receive(match=Match("type", "leave"))
def handle_leave(conn, msg):
    username = getattr(conn, "user", "Anonymous")
    groups = conn.groups()

    if not groups:
        conn.send(
            json.dumps(
                {
                    "type": "error",
                    "message": "You are not in any room",
                }
            )
        )
        return

    room = groups[0]
    conn.leave(room)

    logger.info(f"{username} left room: {room}")

    # Notify the user
    conn.send(
        json.dumps(
            {
                "type": "left",
                "room": room,
                "message": f"You left {room}",
            }
        )
    )

    # Broadcast to room that user left (via Redis for multi-server)
    broadcast(
        [room],
        json.dumps(
            {
                "type": "user_left",
                "username": username,
                "room": room,
            }
        ),
    )


@chat.receive(match=Match("type", "message"))
def handle_message(conn, msg):
    username = getattr(conn, "user", "Anonymous")
    msg = json.loads(msg)
    content = msg.get("message", "")
    groups = conn.groups()

    if not groups:
        conn.send(
            json.dumps(
                {
                    "type": "error",
                    "message": "Join a room first to send messages",
                }
            )
        )
        return

    if not content:
        return

    room = groups[0]

    logger.info(f"{username} in {room}: {content}")

    # Broadcast message to room (via Redis for multi-server)
    broadcast(
        [room],
        json.dumps(
            {
                "type": "message",
                "sender": username,
                "message": content,
                "room": room,
            }
        ),
    )


@chat.receive
def fallback(conn, data):
    conn.send(
        json.dumps(
            {
                "type": "error",
                "message": "Unknown message type",
            }
        )
    )


@chat.disconnect
def on_disconnect(conn, code, reason):
    username = getattr(conn, "user", "Anonymous")
    groups = conn.groups()

    logger.info(f"Client disconnected: {username} (code={code})")

    for room in groups:
        broadcast(
            [room],
            json.dumps(
                {
                    "type": "user_left",
                    "username": username,
                    "room": room,
                }
            ),
        )
