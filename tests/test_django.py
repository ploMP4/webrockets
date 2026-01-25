import asyncio
import json

import pytest
import websockets
from django.db import models
from webrockets import Connection, WebsocketServer
from webrockets.django.auth import SessionAuthentication
from webrockets.test import runserver
from websockets.sync.client import connect as connect_sync


class Message(models.Model):
    content = models.TextField()
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        app_label = "tests"
        db_table = "test_messages"


@pytest.mark.django_db(transaction=True)
class TestDjangoORM:
    def test_orm_create_in_receive_handler(self, ws_server: WebsocketServer):
        route = ws_server.create_route("ws/orm/")

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            content = data if isinstance(data, str) else data.decode()
            Message.objects.create(content=content)
            count = Message.objects.count()
            conn.send(json.dumps({"created": True, "count": count}))

        with runserver(ws_server):
            with connect_sync(f"ws://{ws_server.addr()}/ws/orm/") as ws:
                ws.send("test message")
                response = ws.recv()
                data = json.loads(response)
                assert data["created"] is True
                assert data["count"] >= 1

    def test_orm_query_in_receive_handler(self, ws_server: WebsocketServer):
        """Test querying Django models in receive handler."""
        Message.objects.create(content="existing message 1")
        Message.objects.create(content="existing message 2")

        route = ws_server.create_route("ws/orm/")

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            messages = list(Message.objects.order_by("-id")[:5])
            conn.send(json.dumps([{"id": m.id, "content": m.content} for m in messages]))

        with runserver(ws_server):
            with connect_sync(f"ws://{ws_server.addr()}/ws/orm/") as ws:
                ws.send("query")
                response = ws.recv()
                data = json.loads(response)
                assert len(data) >= 2
                assert any(m["content"] == "existing message 1" for m in data)

    def test_orm_update_in_receive_handler(self, ws_server: WebsocketServer):
        """Test updating Django models in receive handler."""
        msg = Message.objects.create(content="original")

        route = ws_server.create_route("ws/orm/")

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            payload = json.loads(data)
            Message.objects.filter(id=payload["id"]).update(content=payload["content"])
            updated = Message.objects.get(id=payload["id"])
            conn.send(json.dumps({"id": updated.id, "content": updated.content}))

        with runserver(ws_server):
            with connect_sync(f"ws://{ws_server.addr()}/ws/orm/") as ws:
                ws.send(json.dumps({"id": msg.id, "content": "updated"}))
                response = ws.recv()
                data = json.loads(response)
                assert data["content"] == "updated"

    def test_orm_delete_in_receive_handler(self, ws_server: WebsocketServer):
        """Test deleting Django models in receive handler."""
        msg = Message.objects.create(content="to be deleted")

        route = ws_server.create_route("ws/orm/")

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            payload = json.loads(data)
            deleted, _ = Message.objects.filter(id=payload["id"]).delete()
            conn.send(json.dumps({"deleted": deleted}))

        with runserver(ws_server):
            with connect_sync(f"ws://{ws_server.addr()}/ws/orm/") as ws:
                ws.send(json.dumps({"id": msg.id}))
                response = ws.recv()
                data = json.loads(response)
                assert data["deleted"] == 1

        assert not Message.objects.filter(id=msg.id).exists()

    @pytest.mark.asyncio
    async def test_async_orm_operations(self, ws_server: WebsocketServer):
        """Test async ORM operations using sync_to_async."""
        route = ws_server.create_route("ws/orm/")

        @route.receive
        async def receive(conn: Connection, data: str | bytes):
            content = data if isinstance(data, str) else data.decode()

            msg = await Message.objects.acreate(content=content)
            await conn.asend(json.dumps({"id": msg.id, "content": msg.content}))

        with runserver(ws_server):
            async with websockets.connect(f"ws://{ws_server.addr()}/ws/orm/") as ws:
                await ws.send("async message")
                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                data = json.loads(response)
                assert data["content"] == "async message"


@pytest.mark.django_db(transaction=True)
class TestDjangoSessionAuth:
    """Tests for Django session authentication with websocket connections."""

    def test_session_auth_valid_session(
        self, ws_server: WebsocketServer, create_session, active_user
    ):
        session = create_session(user=active_user)
        route = ws_server.create_route("ws/auth/", authentication_classes=[SessionAuthentication()])

        @route.connect("after")
        def after_connect(conn: Connection):
            conn.send(json.dumps({"user_id": conn.user.id, "username": conn.user.username}))

        with runserver(ws_server):
            with connect_sync(
                f"ws://{ws_server.addr()}/ws/auth/",
                additional_headers={"Cookie": f"sessionid={session.session_key}"},
            ) as ws:
                response = ws.recv()
                data = json.loads(response)
                assert data["user_id"] == active_user.id
                assert data["username"] == active_user.username

    def test_session_auth_invalid_session(self, ws_server: WebsocketServer):
        """Test websocket connection rejection with invalid session."""
        route = ws_server.create_route("ws/auth/", authentication_classes=[SessionAuthentication()])

        @route.connect("after")
        def after_connect(conn: Connection):
            conn.send("connected")

        with runserver(ws_server):
            with pytest.raises(websockets.exceptions.InvalidStatus) as exc_info:
                with connect_sync(
                    f"ws://{ws_server.addr()}/ws/auth/",
                    additional_headers={"Cookie": "sessionid=invalid-session"},
                ):
                    pass

            assert exc_info.value.response.status_code == 401

    def test_session_auth_no_session(self, ws_server: WebsocketServer):
        """Test websocket connection rejection without session cookie."""
        route = ws_server.create_route("ws/auth/", authentication_classes=[SessionAuthentication()])

        @route.connect("after")
        def after_connect(conn: Connection):
            conn.send("connected")

        with runserver(ws_server):
            with pytest.raises(websockets.exceptions.InvalidStatus) as exc_info:
                with connect_sync(f"ws://{ws_server.addr()}/ws/auth/"):
                    pass

            assert exc_info.value.response.status_code == 401

    def test_session_auth_inactive_user(
        self, ws_server: WebsocketServer, session_store, inactive_user
    ):
        session = session_store()
        session["_auth_user_id"] = str(inactive_user.pk)
        session["_auth_user_backend"] = "django.contrib.auth.backends.ModelBackend"
        session.create()

        route = ws_server.create_route("ws/auth/", authentication_classes=[SessionAuthentication()])

        @route.connect("after")
        def after_connect(conn: Connection):
            conn.send("connected")

        with runserver(ws_server):
            with pytest.raises(websockets.exceptions.InvalidStatus) as exc_info:
                with connect_sync(
                    f"ws://{ws_server.addr()}/ws/auth/",
                    additional_headers={"Cookie": f"sessionid={session.session_key}"},
                ):
                    pass

            assert exc_info.value.response.status_code == 401

    def test_authenticated_user_in_receive_handler(
        self, ws_server: WebsocketServer, create_session, active_user
    ):
        session = create_session(user=active_user)
        route = ws_server.create_route("ws/auth/", authentication_classes=[SessionAuthentication()])

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            conn.send(
                json.dumps(
                    {
                        "received": data if isinstance(data, str) else data.decode(),
                        "from_user": conn.user.username,
                    }
                )
            )

        with runserver(ws_server):
            with connect_sync(
                f"ws://{ws_server.addr()}/ws/auth/",
                additional_headers={"Cookie": f"sessionid={session.session_key}"},
            ) as ws:
                ws.send("hello")
                response = ws.recv()
                data = json.loads(response)
                assert data["received"] == "hello"
                assert data["from_user"] == active_user.username


@pytest.mark.django_db(transaction=True)
class TestDjangoCacheIntegration:
    def test_cache_set_get_in_handler(self, ws_server: WebsocketServer):
        from django.core.cache import cache

        route = ws_server.create_route("ws/cache/")

        @route.receive
        def receive(conn: Connection, data: str | bytes):
            payload = json.loads(data)
            if payload.get("action") == "set":
                cache.set(payload["key"], payload["value"], timeout=60)
                conn.send(json.dumps({"status": "set"}))
            elif payload.get("action") == "get":
                value = cache.get(payload["key"])
                conn.send(json.dumps({"value": value}))

        with runserver(ws_server):
            with connect_sync(f"ws://{ws_server.addr()}/ws/cache/") as ws:
                ws.send(json.dumps({"action": "set", "key": "ws_test", "value": "cached"}))
                response = ws.recv()
                assert json.loads(response)["status"] == "set"

                ws.send(json.dumps({"action": "get", "key": "ws_test"}))
                response = ws.recv()
                assert json.loads(response)["value"] == "cached"
