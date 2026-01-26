import json

from webrockets.client import connect
from webrockets.django import server
from webrockets.django.test import WebsocketTestCase


class ChatTestCase(WebsocketTestCase):
    def test_connect_receives_welcome_message(self):
        """Test that connecting sends a welcome system message."""
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            response = json.loads(ws.recv())
            self.assertEqual(response["type"], "system")
            self.assertIn("Connected", response["message"])

    def test_join_room(self):
        """Test joining a chat room."""
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            ws.recv()  # welcome message
            ws.send(json.dumps({"type": "join", "room": "test", "username": "alice"}))
            response = json.loads(ws.recv())
            self.assertEqual(response["type"], "joined")
            self.assertEqual(response["room"], "test")
            self.assertEqual(response["message"], "You joined test")

    def test_leave_room(self):
        """Test leaving a chat room."""
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            ws.recv()  # welcome message
            ws.send(json.dumps({"type": "join", "room": "test"}))
            ws.recv()  # joined confirmation
            ws.recv()  # user_joined broadcast

            ws.send(json.dumps({"type": "leave"}))
            response = json.loads(ws.recv())
            self.assertEqual(response["type"], "left")
            self.assertEqual(response["room"], "test")

    def test_leave_without_room_returns_error(self):
        """Test that leaving without being in a room returns an error."""
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            ws.recv()  # welcome message
            ws.send(json.dumps({"type": "leave"}))
            response = json.loads(ws.recv())
            self.assertEqual(response["type"], "error")
            self.assertIn("not in any room", response["message"])

    def test_send_message_to_room(self):
        """Test sending a message broadcasts to room members."""
        with connect(f"ws://{server.addr()}/ws/chat/") as ws1:
            with connect(f"ws://{server.addr()}/ws/chat/") as ws2:
                # Both clients connect and join the same room
                ws1.recv()  # welcome
                ws2.recv()  # welcome

                ws1.send(json.dumps({"type": "join", "room": "chat", "username": "alice"}))
                ws1.recv()  # joined
                ws1.recv()  # user_joined broadcast

                ws2.send(json.dumps({"type": "join", "room": "chat", "username": "bob"}))
                ws2.recv()  # joined
                ws1.recv()  # user_joined for bob
                ws2.recv()  # user_joined broadcast

                # Alice sends a message
                ws1.send(json.dumps({"type": "message", "message": "Hello everyone!"}))

                # Both should recv the broadcast
                msg1 = json.loads(ws1.recv())
                msg2 = json.loads(ws2.recv())

                self.assertEqual(msg1["type"], "message")
                self.assertEqual(msg1["sender"], "alice")
                self.assertEqual(msg1["message"], "Hello everyone!")

                self.assertEqual(msg2["type"], "message")
                self.assertEqual(msg2["sender"], "alice")

    def test_message_without_room_returns_error(self):
        """Test that sending a message without joining a room returns an error."""
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            ws.recv()  # welcome message
            ws.send(json.dumps({"type": "message", "message": "hello"}))
            response = json.loads(ws.recv())
            self.assertEqual(response["type"], "error")
            self.assertIn("Join a room first", response["message"])

    def test_unknown_message_type_returns_error(self):
        """Test that unknown message types return an error."""
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            ws.recv()  # welcome message
            ws.send(json.dumps({"type": "unknown_type"}))
            response = json.loads(ws.recv())
            self.assertEqual(response["type"], "error")
            self.assertIn("Unknown message type", response["message"])

    def test_invalid_json_returns_error(self):
        """Test that invalid JSON returns an error."""
        with connect(f"ws://{server.addr()}/ws/chat/") as ws:
            ws.recv()  # welcome message
            ws.send("not valid json")
            response = json.loads(ws.recv())
            self.assertEqual(response["type"], "error")
