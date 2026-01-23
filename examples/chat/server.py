# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "pydantic",
#   "webrockets"
# ]
# ///

import logging
from typing import Annotated, Any, Literal, cast

from pydantic import BaseModel, StringConstraints
from webrockets import Connection, IncomingConnection, Match, WebsocketServer
from webrockets.auth import AuthenticationFailed, BaseAuthentication

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s: %(message)s")


class ChatAuth(BaseAuthentication):
    def authenticate(self, conn: IncomingConnection) -> Any | None:
        name = conn.get_header("username")
        if name is None:
            raise AuthenticationFailed("username header not found", 4001)
        return name


class JoinMessage(BaseModel):
    type: Literal["join"]
    room: Annotated[str, StringConstraints(max_length=20)]


class JoinMessageResponse(BaseModel):
    type: Literal["join"] = "join"
    user: str


class ChatMessage(BaseModel):
    type: Literal["message"]
    msg: Annotated[str, StringConstraints(max_length=1024)]


class ChatMessageResponse(BaseModel):
    type: Literal["message"] = "message"
    user: str
    msg: Annotated[str, StringConstraints(max_length=1024)]


class LeaveMessageResponse(BaseModel):
    type: Literal["leave"] = "leave"
    user: str


server = WebsocketServer()

chat = server.create_route("chat", authentication_classes=[ChatAuth()])


@chat.connect("after")
def connect(conn: Connection):
    conn.send("Welcome!")


@chat.receive(match=Match("type", "join"), schema=JoinMessage)
def join(conn: Connection, data: JoinMessage):
    if len(conn.groups()) > 0:
        conn.send("You need to leave the current room to join another one")
        return

    conn.join(data.room)
    conn.broadcast([data.room], JoinMessageResponse(user=cast(str, conn.user)).model_dump_json())


@chat.receive(match=Match("type", "leave"))
def leave(conn: Connection, data: str | bytes):
    if len(conn.groups()) < 1:
        conn.send("You are not currently in a group")
        return

    group = conn.groups()[0]
    conn.send(f"Leaving {group}...")
    conn.leave(group)
    conn.broadcast([group], LeaveMessageResponse(user=cast(str, conn.user)).model_dump_json())


@chat.receive(match=Match("type", "message"), schema=ChatMessage)
def message(conn: Connection, data: ChatMessage):
    conn.broadcast(
        conn.groups(),
        ChatMessageResponse(user=cast(str, conn.user), msg=data.msg).model_dump_json(),
        exclude_self=True,
    )


@chat.disconnect
def disconnect(conn: Connection, code: int | None, reason: str | None):
    conn.broadcast(
        conn.groups(),
        LeaveMessageResponse(user=cast(str, conn.user)).model_dump_json(),
        exclude_self=True,
    )


try:
    server.start()
except KeyboardInterrupt:
    pass
