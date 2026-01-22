---
title: Broadcasting
description: Send messages from Django to WebSocket clients.
---

When using webrockets with Django, the WebSocket server runs as a **separate process** from your Django application. This means you need a message broker to send messages from Django to connected WebSocket clients.

## Understanding the Architecture

```
┌─────────────────┐         ┌─────────────────┐
│     Django      │         │  webrockets     │
│   (gunicorn)    │         │  WebSocket      │
│   port 8000     │         │  port 46290     │
└────────┬────────┘         └────────┬────────┘
         │                           │
         │      broadcast()          │
         └───────────┬───────────────┘
                     │
              ┌──────▼──────┐
              │    Redis    │
              │  (broker)   │
              └─────────────┘
```

:::caution[Important Distinction]
- **`conn.broadcast()`** - Only sends to clients connected to the **same server process**
- **`broadcast()`** function - Sends via the broker to clients on **all server processes**

Since Django and webrockets are separate processes, you **must** use the `broadcast()` function to send messages from Django.
:::

## Setup

### 1. Configure a Broker

A message broker is **required** for Django to communicate with the WebSocket server. Configure it in your Django settings:

**Redis (recommended):**

```python
# settings.py
WEBSOCKET_BROKER = {
    "type": "redis",
    "url": "redis://localhost:6379",
}
```

**RabbitMQ:**

```python
# settings.py
WEBSOCKET_BROKER = {
    "type": "amqp",
    "url": "amqp://guest:guest@localhost:5672",
}
```

### 2. Subscribe Clients to Groups

In your WebSocket handlers, subscribe clients to groups they should receive messages for:

```python
# myapp/websockets.py
from webrockets.django import server
from webrockets.django.auth import SessionAuthentication

chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[SessionAuthentication()]
)

@chat.connect("before")
def on_connect(conn):
    # Subscribe to user-specific and room groups
    conn.join(f"user_{conn.user.id}")
```

### 3. Broadcast from Django

Use the `broadcast()` function to send messages from anywhere in Django:

```python
from webrockets import broadcast

# Send to all clients subscribed to a group
broadcast(["chat"], '{"type": "message", "content": "Hello!"}')
```

## Broadcasting from Django

### From Views

```python
from django.http import JsonResponse
from webrockets import broadcast

def send_notification(request):
    broadcast(
        [f"user_{request.user.id}"],
        '{"type": "notification", "message": "You have a new message!"}'
    )
    return JsonResponse({"status": "sent"})
```

### From Signals

```python
from django.db.models.signals import post_save
from django.dispatch import receiver
from webrockets import broadcast
import json

@receiver(post_save, sender=Order)
def order_created(sender, instance, created, **kwargs):
    if created:
        broadcast(
            [f"user_{instance.user_id}"],
            json.dumps({
                "type": "order_created",
                "order_id": instance.id,
                "total": str(instance.total),
            })
        )
```

### From Celery Tasks

```python
from celery import shared_task
from webrockets import broadcast
import json

@shared_task
def notify_users(user_ids, message):
    for user_id in user_ids:
        broadcast(
            [f"user_{user_id}"],
            json.dumps({"type": "notification", "message": message})
        )

@shared_task
def broadcast_to_room(room_id, message):
    broadcast(
        [f"room_{room_id}"],
        json.dumps({"type": "room_message", "content": message})
        )
```

### From Management Commands

```python
from django.core.management.base import BaseCommand
from webrockets import broadcast
import json

class Command(BaseCommand):
    help = "Send maintenance notification to all users"

    def handle(self, *args, **options):
        broadcast(
            ["announcements"],
            json.dumps({
                "type": "maintenance",
                "message": "Server restart in 5 minutes"
            })
        )
        self.stdout.write(self.style.SUCCESS("Notification sent"))
```

## WebSocket Handler Broadcasting

Within WebSocket handlers, you have two options:

### `conn.broadcast()` - Same Server Only

```python
@chat.receive
def on_message(conn, data):
    # Only sends to clients connected to THIS server process
    # Does NOT go through the broker
    conn.broadcast(["room_general"], data)
```

Use this when:
- Running a single webrockets instance
- You only need to reach clients on the same server

### `broadcast()` - All Servers

```python
from webrockets import broadcast

@chat.receive
def on_message(conn, data):
    # Sends through the broker to ALL server processes
    broadcast(["room_general"], data)
```

Use this when:
- Running multiple webrockets instances
- You want consistent behavior regardless of deployment

:::tip[Recommendation]
For Django deployments, always use the `broadcast()` function for consistency. This ensures your code works the same whether you're running one server or many.
:::

## Broker Configuration Options

### Redis

```python
WEBSOCKET_BROKER = {
    "type": "redis",
    "url": "redis://localhost:6379",  # Connection URL
    "channel": "ws_broadcast",         # Pub/sub channel (optional)
}
```

| Option | Default | Description |
|--------|---------|-------------|
| `type` | - | Must be `"redis"` |
| `url` | `"redis://localhost:6379"` | Redis connection URL |
| `channel` | `"ws_broadcast"` | Redis pub/sub channel name |

### RabbitMQ

```python
WEBSOCKET_BROKER = {
    "type": "amqp",
    "url": "amqp://guest:guest@localhost:5672",
    "exchange": "ws_broadcast",  # Exchange name (optional)
    "queue": None,               # Queue name, auto-generated if None
    "routing_key": "#",          # Routing key pattern (optional)
}
```

| Option | Default | Description |
|--------|---------|-------------|
| `type` | - | Must be `"amqp"` |
| `url` | `"amqp://localhost:5672"` | AMQP connection URL |
| `exchange` | `"ws_broadcast"` | Exchange name |
| `queue` | Auto-generated UUID | Queue name for this server |
| `routing_key` | `"#"` | Routing key pattern |

## Complete Example

A chat application with rooms and direct messaging:

```python
# myapp/websockets.py
import json
from webrockets.django import server
from webrockets.django.auth import SessionAuthentication
from webrockets import broadcast

chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[SessionAuthentication()]
)

@chat.connect("before")
def on_connect(conn):
    conn.join(f"user_{conn.user.id}")

    # Notify room (via broker for multi-server support)
    broadcast(
        ["chat"],
        json.dumps({
            "type": "user_joined",
            "user": conn.user.username
        })
    )

@chat.receive
def on_message(conn, data):
    msg = json.loads(data)

    if msg.get("type") == "chat":
        # Broadcast chat message to room
        broadcast(
            ["chat"],
            json.dumps({
                "type": "message",
                "user": conn.user.username,
                "content": msg["content"]
            })
        )

    elif msg.get("type") == "dm":
        # Direct message to specific user
        broadcast(
            [f"user_{msg['to_user_id']}"],
            json.dumps({
                "type": "dm",
                "from": conn.user.username,
                "content": msg["content"]
            })
        )

@chat.disconnect
def on_disconnect(conn, code=None, reason=None):
    if hasattr(conn, "room"):
        broadcast(
            ["chat"],
            json.dumps({
                "type": "user_left",
                "user": conn.user.username
            })
        )
```

```python
# myapp/views.py
from django.http import JsonResponse
from django.views.decorators.http import require_POST
from webrockets import broadcast
import json

@require_POST
def send_announcement(request):
    """Admin endpoint to send announcement to a room."""
    data = json.loads(request.body)

    broadcast(
        ["chat"],
        json.dumps({
            "type": "announcement",
            "message": data["message"]
        })
    )

    return JsonResponse({"status": "sent"})
```

## Next Steps

- [Deployment Overview](/django/deployment/overview/) - Deploy to production
- [Authentication](/django/authentication/) - Configure user authentication
