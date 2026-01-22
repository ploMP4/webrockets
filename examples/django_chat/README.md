# Django Chat Example

A complete example showcasing webrockets integration with Django, featuring:

- Django web application with webrockets WebSocket server
- Redis broker for multi-server broadcasting
- Nginx reverse proxy routing HTTP to Django and WebSocket to webrockets
- Real-time chat with rooms
- HTTP API endpoint that broadcasts to WebSocket clients

## Running

Start all services with Docker Compose:

```bash
docker compose up --build
```

Then open http://localhost:8080 in your browser.

## Features

### Real-time Chat

1. Enter a username and room name
2. Click "Join Room" to connect
3. Send messages to everyone in the room
4. Open multiple browser tabs to simulate multiple users

### Django API Broadcasting

The example includes an HTTP API endpoint that broadcasts messages to WebSocket clients:

```bash
curl -X POST http://localhost:8080/api/broadcast/ \
  -H "Content-Type: application/json" \
  -d '{"room": "general", "message": "Hello from the API!", "sender": "Server"}'
```

This demonstrates how Django views, Celery tasks, or signals can send real-time updates to connected WebSocket clients via the Redis broker.

## Project Structure

```
django_chat/
├── docker-compose.yml      # Docker Compose configuration
├── Dockerfile              # Container image definition
├── requirements.txt        # Python dependencies
├── manage.py               # Django management script
├── nginx/
│   └── nginx.conf          # Nginx reverse proxy config
├── myproject/
│   ├── settings.py         # Django settings with webrockets config
│   ├── urls.py             # URL routing
│   └── wsgi.py             # WSGI application
├── chat/
│   ├── views.py            # Django views (index, API endpoint)
│   └── websockets.py       # webrockets WebSocket routes
└── templates/
    └── index.html          # Chat interface
```

## Key Files

### websockets.py

The WebSocket routes are defined in `chat/websockets.py` and automatically discovered by webrockets:

```python
from webrockets import broadcast
from webrockets.django import server

chat = server.create_route("ws/chat/", "chat")

@chat.receive
def on_message(conn, data):
    # Broadcast via Redis to all servers
    broadcast([room], json.dumps({"type": "message", ...}))
```

### Django Settings

```python
# Redis broker for multi-server broadcasting
WEBSOCKET_BROKER = {
    "type": "redis",
    "url": os.environ.get("REDIS_URL", "redis://localhost:6379"),
}
```

### API Broadcasting

```python
from webrockets import broadcast

def broadcast_message(request):
    # Send message to WebSocket clients from Django
    broadcast(["room"], json.dumps({"type": "message", ...}))
```
