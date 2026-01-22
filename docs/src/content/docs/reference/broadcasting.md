---
title: Broadcasting
description: API reference for external broadcasting functions.
---

webrockets provides standalone functions for broadcasting messages from outside WebSocket handlers, such as Django views, Celery tasks, or other backend processes.

## Import

```python
from webrockets import setup_broadcast, broadcast, abroadcast, reset_broadcast
```

## Functions

### setup_broadcast

Initialize the broadcaster with a broker configuration. Must be called before using `broadcast()` or `abroadcast()`.

```python
setup_broadcast(config: BrokerConfig) -> None
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `config` | `BrokerConfig` | Broker configuration (Redis or AMQP) |

#### Example

```python
from webrockets import setup_broadcast

# Redis configuration
setup_broadcast({
    "type": "redis",
    "url": "redis://localhost:6379",
    "channel": "ws_broadcast"
})

# RabbitMQ configuration
setup_broadcast({
    "type": "amqp",
    "url": "amqp://localhost:5672",
    "exchange": "ws_broadcast"
})
```

### broadcast

Publish a message to the specified groups via the configured broker. This is a blocking (synchronous) call.

```python
broadcast(groups: list[str], message: str) -> None
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `groups` | `list[str]` | List of group names to broadcast to |
| `message` | `str` | The message payload (typically a JSON string) |

#### Example

```python
from webrockets import broadcast
import json

# In a Django view
def send_notification(request):
    message = json.dumps({
        "type": "notification",
        "text": "New message received!"
    })
    broadcast(["notifications"], message)
    return JsonResponse({"status": "sent"})
```

### abroadcast

Async version of `broadcast()`. Publish a message to the specified groups via the configured broker.

```python
async abroadcast(groups: list[str], message: str) -> None
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `groups` | `list[str]` | List of group names to broadcast to |
| `message` | `str` | The message payload (typically a JSON string) |

#### Example

```python
from webrockets import abroadcast
import json

# In an async Django view
async def send_notification(request):
    message = json.dumps({
        "type": "notification",
        "text": "New message received!"
    })
    await abroadcast(["notifications"], message)
    return JsonResponse({"status": "sent"})
```

### reset_broadcast

Reset the broadcaster, allowing `setup_broadcast()` to be called again. Primarily useful for testing when switching between broker configurations.

```python
reset_broadcast() -> None
```

#### Example

```python
from webrockets import setup_broadcast, reset_broadcast, broadcast

# In tests
def test_redis_broadcast():
    setup_broadcast({"type": "redis"})
    broadcast(["test"], "message")
    reset_broadcast()  # Clean up for next test

def test_amqp_broadcast():
    setup_broadcast({"type": "amqp"})
    broadcast(["test"], "message")
    reset_broadcast()
```

## Broker Configuration

### Redis

```python
{
    "type": "redis",
    "url": "redis://localhost:6379",  # optional, default shown
    "channel": "ws_broadcast"          # optional, default shown
}
```

### RabbitMQ (AMQP)

```python
{
    "type": "amqp",
    "url": "amqp://localhost:5672",    # optional, default shown
    "exchange": "ws_broadcast",         # optional, default shown
    "queue": None,                      # optional, auto-generated UUID if None
    "routing_key": "#"                  # optional, default shown
}
```

## Complete Example

### Django Integration

```python
# settings.py
WEBSOCKET_BROKER = {
    "type": "redis",
    "url": "redis://localhost:6379"
}

# apps.py
from django.apps import AppConfig

class MyAppConfig(AppConfig):
    name = "myapp"

    def ready(self):
        from django.conf import settings
        from webrockets import setup_broadcast

        if hasattr(settings, "WEBSOCKET_BROKER"):
            setup_broadcast(settings.WEBSOCKET_BROKER)
```

### Broadcasting from Views

```python
# views.py
from django.http import JsonResponse
from webrockets import broadcast
import json

def create_post(request):
    # ... create post logic ...

    # Notify all connected clients
    broadcast(
        ["posts"],
        json.dumps({
            "type": "new_post",
            "post_id": post.id,
            "title": post.title
        })
    )

    return JsonResponse({"id": post.id})
```

### Broadcasting from Celery Tasks

```python
# tasks.py
from celery import shared_task
from webrockets import broadcast
import json

@shared_task
def process_order(order_id):
    # ... process order logic ...

    # Notify the user's channel
    broadcast(
        [f"user_{order.user_id}"],
        json.dumps({
            "type": "order_status",
            "order_id": order_id,
            "status": "completed"
        })
    )
```

### Async Broadcasting

```python
# async_views.py
from django.http import JsonResponse
from webrockets import abroadcast
import json

async def async_create_post(request):
    # ... async create post logic ...

    await abroadcast(
        ["posts"],
        json.dumps({
            "type": "new_post",
            "post_id": post.id
        })
    )

    return JsonResponse({"id": post.id})
```
