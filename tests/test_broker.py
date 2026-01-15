import asyncio
import json
import threading
import time

import pytest
import websockets
from pywsrs import (
    BrokerConfig,
    WebsocketServer,
    abroadcast,
    broadcast,
    setup_broadcast,
)
from testcontainers.redis import RedisContainer


@pytest.fixture(scope="module")
def redis_container():
    with RedisContainer("redis:7-alpine") as redis:
        yield redis


@pytest.fixture(scope="module")
def redis_ws_server(redis_container):
    redis_url = f"redis://{redis_container.get_container_host_ip()}:{redis_container.get_exposed_port(6379)}"

    broker_config: BrokerConfig = {
        "type": "redis",
        "url": redis_url,
        "channel": "ws_test_broadcast",
    }

    setup_broadcast(broker_config)
    server = WebsocketServer(host="127.0.0.1", port=46391, broker=broker_config)
    redis_view = server.create_route("ws/redis-test/", "redis-test")

    @redis_view.receive
    def on_redis_receive(conn, data):
        conn.send(f"redis-ack: {data}")

    server_thread = threading.Thread(target=lambda: server.start())
    server_thread.start()
    time.sleep(0.5)

    yield {
        "ws_url": "ws://127.0.0.1:46391",
        "redis_url": redis_url,
        "channel": "ws_test_broadcast",
    }

    server.stop()
    server_thread.join(timeout=5)


class TestRedisBroker:
    @pytest.mark.asyncio
    async def test_redis_broadcaster_send(self, redis_ws_server):
        async with websockets.connect(f"{redis_ws_server['ws_url']}/ws/redis-test/") as ws:
            await asyncio.sleep(0.2)

            broadcast(
                groups=["redis-test"],
                message=json.dumps({"type": "test", "data": "hello from redis"}),
            )

            try:
                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert "hello from redis" in str(response)
            except asyncio.TimeoutError:
                pytest.fail("Did not receive broadcast message within timeout")

    @pytest.mark.asyncio
    async def test_redis_broadcaster_asend(self, redis_ws_server):
        async with websockets.connect(f"{redis_ws_server['ws_url']}/ws/redis-test/") as ws:
            await asyncio.sleep(0.2)

            await abroadcast(
                groups=["redis-test"],
                message=json.dumps({"type": "async_test", "data": "async hello"}),
            )

            try:
                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert "async hello" in str(response)
            except asyncio.TimeoutError:
                pytest.fail("Did not receive async broadcast message within timeout")

    @pytest.mark.asyncio
    async def test_redis_multiple_broadcasts(self, redis_ws_server):
        async with websockets.connect(f"{redis_ws_server['ws_url']}/ws/redis-test/") as ws:
            await asyncio.sleep(0.2)

            messages = ["first", "second", "third"]
            for msg in messages:
                broadcast(groups=["redis-test"], message=msg)

            received = []
            for _ in range(len(messages)):
                try:
                    response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                    received.append(response)
                except asyncio.TimeoutError:
                    break

            assert len(received) == len(messages)
            for msg in messages:
                assert any(msg in r for r in received)

    @pytest.mark.asyncio
    async def test_redis_broadcast_to_multiple_clients(self, redis_ws_server):
        async with websockets.connect(f"{redis_ws_server['ws_url']}/ws/redis-test/") as ws1:
            async with websockets.connect(f"{redis_ws_server['ws_url']}/ws/redis-test/") as ws2:
                await asyncio.sleep(0.2)

                broadcast(groups=["redis-test"], message="broadcast to all")

                try:
                    r1 = await asyncio.wait_for(ws1.recv(), timeout=2.0)
                    r2 = await asyncio.wait_for(ws2.recv(), timeout=2.0)

                    assert "broadcast to all" in str(r1)
                    assert "broadcast to all" in str(r2)
                except asyncio.TimeoutError:
                    pytest.fail("Not all clients received the broadcast")


class TestBroadcasterErrors:
    def test_broadcaster_invalid_type(self):
        with pytest.raises(RuntimeError, match="unknown broker type"):
            setup_broadcast({"type": "invalid"})
