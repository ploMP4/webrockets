import asyncio
import json

import pytest
import websockets
from testcontainers.rabbitmq import RabbitMqContainer
from testcontainers.redis import RedisContainer
from webrockets import (
    BrokerConfig,
    Connection,
    WebsocketServer,
    abroadcast,
    broadcast,
    reset_broadcast,
    setup_broadcast,
)
from webrockets.test import runserver


@pytest.fixture(scope="module")
def redis_container():
    with RedisContainer("redis:7-alpine") as redis:
        yield redis


@pytest.fixture
def broker_config(redis_container) -> BrokerConfig:
    redis_url = f"redis://{redis_container.get_container_host_ip()}:{redis_container.get_exposed_port(6379)}"
    config: BrokerConfig = {
        "type": "redis",
        "url": redis_url,
        "channel": "ws_test_broadcast",
    }
    setup_broadcast(config)
    return config


@pytest.fixture
def redis_ws_server(broker_config: BrokerConfig) -> WebsocketServer:
    server = WebsocketServer(broker=broker_config)
    route = server.create_route("ws/redis-test/", "redis-test")

    @route.receive
    def on_receive(conn: Connection, data: str | bytes):
        conn.send(f"redis-ack: {data}")

    return server


class TestRedisBroker:
    @pytest.mark.asyncio
    async def test_redis_broadcaster_send(self, redis_ws_server: WebsocketServer):
        with runserver(redis_ws_server):
            async with websockets.connect(f"ws://{redis_ws_server.addr()}/ws/redis-test/") as ws:
                broadcast(
                    groups=["redis-test"],
                    message=json.dumps({"type": "test", "data": "hello from redis"}),
                )

                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert "hello from redis" in str(response)

    @pytest.mark.asyncio
    async def test_redis_broadcaster_asend(self, redis_ws_server: WebsocketServer):
        with runserver(redis_ws_server):
            async with websockets.connect(f"ws://{redis_ws_server.addr()}/ws/redis-test/") as ws:
                await abroadcast(
                    groups=["redis-test"],
                    message=json.dumps({"type": "async_test", "data": "async hello"}),
                )

                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert "async hello" in str(response)

    @pytest.mark.asyncio
    async def test_redis_multiple_broadcasts(self, redis_ws_server: WebsocketServer):
        with runserver(redis_ws_server):
            async with websockets.connect(f"ws://{redis_ws_server.addr()}/ws/redis-test/") as ws:
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
    async def test_redis_broadcast_to_multiple_clients(self, redis_ws_server: WebsocketServer):
        with runserver(redis_ws_server):
            async with websockets.connect(f"ws://{redis_ws_server.addr()}/ws/redis-test/") as ws1:
                async with websockets.connect(
                    f"ws://{redis_ws_server.addr()}/ws/redis-test/"
                ) as ws2:
                    broadcast(groups=["redis-test"], message="broadcast to all")

                    r1 = await asyncio.wait_for(ws1.recv(), timeout=2.0)
                    r2 = await asyncio.wait_for(ws2.recv(), timeout=2.0)

                    assert "broadcast to all" in str(r1)
                    assert "broadcast to all" in str(r2)

    @pytest.mark.asyncio
    async def test_redis_broadcast_works_from_handler(self, redis_ws_server: WebsocketServer):
        route = redis_ws_server.create_route("ws/redis-test-broadcast/", "redis-test")

        @route.receive
        def on_receive(conn: Connection, data: str | bytes):
            broadcast(groups=["redis-test"], message="broadcast to all")

        with runserver(redis_ws_server):
            async with websockets.connect(
                f"ws://{redis_ws_server.addr()}/ws/redis-test-broadcast/"
            ) as ws:
                await ws.send("hi")
                r = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert "broadcast to all" in str(r)


@pytest.fixture(scope="module")
def amqp_container():
    with RabbitMqContainer("rabbitmq:3-alpine") as rabbitmq:
        yield rabbitmq


@pytest.fixture
def amqp_broker_config(amqp_container) -> BrokerConfig:
    amqp_url = f"amqp://guest:guest@{amqp_container.get_container_host_ip()}:{amqp_container.get_exposed_port(5672)}"
    config: BrokerConfig = {
        "type": "amqp",
        "url": amqp_url,
        "exchange": "ws_test_broadcast",
    }
    reset_broadcast()
    setup_broadcast(config)
    return config


@pytest.fixture
def amqp_ws_server(amqp_broker_config: BrokerConfig) -> WebsocketServer:
    server = WebsocketServer(broker=amqp_broker_config)
    route = server.create_route("ws/amqp-test/", "amqp-test")

    @route.receive
    def on_receive(conn: Connection, data: str | bytes):
        conn.send(f"amqp-ack: {data}")

    return server


class TestAmqpBroker:
    @pytest.mark.asyncio
    async def test_amqp_broadcaster_send(self, amqp_ws_server: WebsocketServer):
        with runserver(amqp_ws_server):
            async with websockets.connect(f"ws://{amqp_ws_server.addr()}/ws/amqp-test/") as ws:
                broadcast(
                    groups=["amqp-test"],
                    message=json.dumps({"type": "test", "data": "hello from amqp"}),
                )

                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert "hello from amqp" in str(response)

    @pytest.mark.asyncio
    async def test_amqp_broadcaster_asend(self, amqp_ws_server: WebsocketServer):
        with runserver(amqp_ws_server):
            async with websockets.connect(f"ws://{amqp_ws_server.addr()}/ws/amqp-test/") as ws:
                await abroadcast(
                    groups=["amqp-test"],
                    message=json.dumps({"type": "async_test", "data": "async hello"}),
                )

                response = await asyncio.wait_for(ws.recv(), timeout=2.0)
                assert "async hello" in str(response)

    @pytest.mark.asyncio
    async def test_amqp_multiple_broadcasts(self, amqp_ws_server: WebsocketServer):
        with runserver(amqp_ws_server):
            async with websockets.connect(f"ws://{amqp_ws_server.addr()}/ws/amqp-test/") as ws:
                messages = ["first", "second", "third"]
                for msg in messages:
                    broadcast(groups=["amqp-test"], message=msg)

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
    async def test_amqp_broadcast_to_multiple_clients(self, amqp_ws_server: WebsocketServer):
        with runserver(amqp_ws_server):
            async with websockets.connect(f"ws://{amqp_ws_server.addr()}/ws/amqp-test/") as ws1:
                async with websockets.connect(f"ws://{amqp_ws_server.addr()}/ws/amqp-test/") as ws2:
                    broadcast(groups=["amqp-test"], message="broadcast to all")

                    r1 = await asyncio.wait_for(ws1.recv(), timeout=2.0)
                    r2 = await asyncio.wait_for(ws2.recv(), timeout=2.0)

                    assert "broadcast to all" in str(r1)
                    assert "broadcast to all" in str(r2)


class TestBroadcasterErrors:
    def test_broadcaster_invalid_type(self):
        with pytest.raises(ValueError, match="unknown broker type"):
            setup_broadcast({"type": "invalid"})
