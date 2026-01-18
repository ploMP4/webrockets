from pywsrs import Connection, WebsocketServer

server = WebsocketServer()

echo = server.create_route("echo")


@echo.receive
def receive(conn: Connection, data: str | bytes):
    conn.send(data)


server.start()
