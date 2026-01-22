---
title: Nginx
description: Configure nginx as a reverse proxy for Django and webrockets with Docker.
---

This guide shows how to configure nginx with Docker to route HTTP requests to Django and WebSocket connections to webrockets.

## Docker Compose Setup

A complete Docker Compose setup with nginx, Django, webrockets, and Redis:

```yaml
# docker-compose.yml
services:
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./static:/var/www/static:ro
      - ./certs:/etc/nginx/certs:ro  # For SSL certificates
    depends_on:
      - django
      - websocket

  django:
    build: .
    command: gunicorn myproject.wsgi:application --bind 0.0.0.0:8000 --workers 4
    environment:
      - DJANGO_SETTINGS_MODULE=myproject.settings.production
      - DATABASE_URL=postgres://user:pass@db/myproject
      - REDIS_URL=redis://redis:6379
      - SECRET_KEY=${SECRET_KEY}
    depends_on:
      - db
      - redis

  websocket:
    build: .
    command: python manage.py runwebsockets --log-level info
    environment:
      - DJANGO_SETTINGS_MODULE=myproject.settings.production
      - DATABASE_URL=postgres://user:pass@db/myproject
      - REDIS_URL=redis://redis:6379
      - SECRET_KEY=${SECRET_KEY}
    depends_on:
      - db
      - redis

  redis:
    image: redis:latest

  db:
    image: postgres:latest
    environment:
      - POSTGRES_USER=user
      - POSTGRES_PASSWORD=pass
      - POSTGRES_DB=myproject
    volumes:
      - postgres_data:/var/lib/postgresql/data

volumes:
  postgres_data:
```

## Nginx Configuration

```nginx
# nginx.conf
events {
    worker_connections 1024;
}

http {
    include       mime.types;
    default_type  application/octet-stream;

    # Upstream servers
    upstream django {
        server django:8000;
        keepalive 32;
    }

    upstream websocket {
        server websocket:46290;
        keepalive 32;
    }

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=ws_limit:10m rate=10r/s;

    server {
        listen 80;
        server_name _;

        # WebSocket routes
        location /ws/ {
            limit_req zone=ws_limit burst=20 nodelay;

            proxy_pass http://websocket;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;

            # WebSocket timeouts
            proxy_read_timeout 86400;
            proxy_send_timeout 86400;
            proxy_buffering off;
        }

        # Static files
        location /static/ {
            alias /var/www/static/;
            expires 30d;
            add_header Cache-Control "public, immutable";
        }

        # Django application
        location / {
            proxy_pass http://django;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
    }
}
```

## Configuration Explained

### Docker Service Names

In Docker Compose, services communicate using their service names. The upstream servers use the service names `django` and `websocket`:

```nginx
upstream django {
    server django:8000;
}

upstream websocket {
    server websocket:46290;
}
```

### WebSocket Proxy

The WebSocket location requires special headers for the upgrade handshake:

```nginx
location /ws/ {
    proxy_pass http://websocket;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    # ...
}
```

| Header | Purpose |
|--------|---------|
| `Upgrade` | Passes the WebSocket upgrade request |
| `Connection "upgrade"` | Signals connection upgrade |
| `proxy_http_version 1.1` | Required for WebSocket |

### Timeouts

WebSocket connections are long-lived. Increase timeouts:

```nginx
proxy_read_timeout 86400;  # 24 hours
proxy_send_timeout 86400;
```

## With SSL/TLS

For production with HTTPS:

```nginx
# nginx.conf
events {
    worker_connections 1024;
}

http {
    include       mime.types;
    default_type  application/octet-stream;

    upstream django {
        server django:8000;
        keepalive 32;
    }

    upstream websocket {
        server websocket:46290;
        keepalive 32;
    }

    limit_req_zone $binary_remote_addr zone=ws_limit:10m rate=10r/s;

    # Redirect HTTP to HTTPS
    server {
        listen 80;
        server_name example.com;
        return 301 https://$server_name$request_uri;
    }

    # HTTPS server
    server {
        listen 443 ssl http2;
        server_name example.com;

        # SSL certificates
        ssl_certificate /etc/nginx/certs/fullchain.pem;
        ssl_certificate_key /etc/nginx/certs/privkey.pem;
        ssl_protocols TLSv1.2 TLSv1.3;
        ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256;
        ssl_prefer_server_ciphers off;

        # HSTS
        add_header Strict-Transport-Security "max-age=63072000" always;

        # WebSocket
        location /ws/ {
            limit_req zone=ws_limit burst=20 nodelay;

            proxy_pass http://websocket;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;

            proxy_read_timeout 86400;
            proxy_send_timeout 86400;
            proxy_buffering off;
        }

        # Static files
        location /static/ {
            alias /var/www/static/;
            expires 30d;
            add_header Cache-Control "public, immutable";
            gzip_static on;
        }

        # Django
        location / {
            proxy_pass http://django;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
    }
}
```
