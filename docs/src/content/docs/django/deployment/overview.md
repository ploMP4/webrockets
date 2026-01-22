---
title: Deployment Overview
description: Deploy webrockets WebSocket server with Django in production.
---

This guide covers deploying webrockets with Django in production. The WebSocket server runs as a **separate process** from Django, which has important implications for how you send messages.

## Architecture

In production, you'll run:

1. **Django application** - Handles HTTP requests (views, REST API, admin, etc.)
2. **webrockets WebSocket server** - Handles WebSocket connections
3. **Message broker** - Enables communication between Django and webrockets
4. **Reverse proxy** - Routes traffic to the appropriate backend

```
                    ┌─────────────────┐
                    │  Reverse Proxy  │
                    │  (nginx/traefik)│
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
              ▼                             ▼
    ┌─────────────────┐          ┌─────────────────┐
    │     Django      │          │    webrockets   │
    │   (gunicorn)    │          │    WebSocket    │
    │   port 8000     │          │    port 46290   │
    └────────┬────────┘          └────────┬────────┘
             │                            │
             └────────────┬───────────────┘
                          │
                   ┌──────▼──────┐
                   │    Redis    │
                   │  (broker)   │
                   └─────────────┘
```

:::caution[Broker Required]
Since Django and webrockets run as separate processes, a **message broker is required** to send messages from Django to WebSocket clients. Without a broker, Django cannot communicate with the WebSocket server.

See the [Broadcasting guide](/django/broadcasting/) for details.
:::

## Routing Strategy

The reverse proxy routes requests based on path or upgrade headers:

| Request Type | Path Pattern | Backend |
|--------------|--------------|---------|
| HTTP | `/api/*`, `/admin/*`, etc. | Django (gunicorn) |
| WebSocket | `/ws/*` | webrockets |

## Quick Start

1. **Configure the broker in Django settings:**
   ```python
   # settings.py
   WEBSOCKET_BROKER = {
       "type": "redis",
       "url": "redis://localhost:6379",
   }
   ```

2. **Run Redis:**
   ```bash
   docker run -d -p 6379:6379 redis:latest
   ```

3. **Run Django with gunicorn:**
   ```bash
   gunicorn myproject.wsgi:application --bind 0.0.0.0:8000
   ```

4. **Run webrockets:**
   ```bash
   python manage.py runwebsockets
   ```

5. **Configure reverse proxy** - See [Nginx](/django/deployment/nginx/) or [Traefik](/django/deployment/traefik/) guides.

## Next Steps

- [Nginx](/django/deployment/nginx/) - Configure nginx as reverse proxy
- [Traefik](/django/deployment/traefik/) - Configure Traefik for Docker/Kubernetes
