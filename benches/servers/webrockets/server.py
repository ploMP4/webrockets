from webrockets import Connection, WebsocketServer

server = WebsocketServer(host="0.0.0.0", port=6969)

echo = server.create_route(path="echo", default_group="echo")


@echo.receive
def echo_receive(conn: Connection, data: str | bytes):
    conn.send(data)


if __name__ == "__main__":
    server.start()
