import asyncio

from websockets.asyncio.server import serve


async def router(websocket):
    async for message in websocket:
        await websocket.send(message)


async def main():
    async with serve(router, "0.0.0.0", 4200, backlog=100_000) as server:
        await server.serve_forever()


if __name__ == "__main__":
    asyncio.run(main())
