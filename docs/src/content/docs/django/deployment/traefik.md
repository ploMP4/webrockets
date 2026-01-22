---
title: Traefik
description: Configure Traefik as a reverse proxy for Django and webrockets.
---

Traefik is a modern reverse proxy well-suited for Docker and Kubernetes deployments. It provides automatic HTTPS, service discovery, and dynamic configuration.

## Docker Compose

A complete Docker Compose setup with Traefik, Django, and webrockets:

```yaml
# docker-compose.yml
services:
  traefik:
    image: traefik:v3.6
    command:
      - "--api.dashboard=true"
      - "--providers.docker=true"
      - "--providers.docker.exposedbydefault=false"
      - "--entrypoints.web.address=:80"
      - "--entrypoints.websecure.address=:443"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge=true"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge.entrypoint=web"
      - "--certificatesresolvers.letsencrypt.acme.email=admin@example.com"
      - "--certificatesresolvers.letsencrypt.acme.storage=/letsencrypt/acme.json"
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - letsencrypt:/letsencrypt
    labels:
      # Dashboard (optional)
      - "traefik.enable=true"
      - "traefik.http.routers.dashboard.rule=Host(`traefik.example.com`)"
      - "traefik.http.routers.dashboard.service=api@internal"
      - "traefik.http.routers.dashboard.entrypoints=websecure"
      - "traefik.http.routers.dashboard.tls.certresolver=letsencrypt"

  django:
    build: .
    command: gunicorn myproject.wsgi:application --bind 0.0.0.0:8000
    environment:
      - DATABASE_URL=postgres://user:pass@db/myproject
      - REDIS_URL=redis://redis:6379
    labels:
      - "traefik.enable=true"
      # HTTP router
      - "traefik.http.routers.django.rule=Host(`example.com`)"
      - "traefik.http.routers.django.entrypoints=websecure"
      - "traefik.http.routers.django.tls.certresolver=letsencrypt"
      - "traefik.http.services.django.loadbalancer.server.port=8000"
      # Redirect HTTP to HTTPS
      - "traefik.http.routers.django-http.rule=Host(`example.com`)"
      - "traefik.http.routers.django-http.entrypoints=web"
      - "traefik.http.routers.django-http.middlewares=https-redirect"
      - "traefik.http.middlewares.https-redirect.redirectscheme.scheme=https"

  websocket:
    build: .
    command: python manage.py runwebsockets
    environment:
      - DATABASE_URL=postgres://user:pass@db/myproject
      - REDIS_URL=redis://redis:6379
    labels:
      - "traefik.enable=true"
      # WebSocket router
      - "traefik.http.routers.websocket.rule=Host(`example.com`) && PathPrefix(`/ws/`)"
      - "traefik.http.routers.websocket.entrypoints=websecure"
      - "traefik.http.routers.websocket.tls.certresolver=letsencrypt"
      - "traefik.http.services.websocket.loadbalancer.server.port=46290"
      # Priority to match before django
      - "traefik.http.routers.websocket.priority=100"

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
  letsencrypt:
  postgres_data:
```

## Configuration Explained

### Routers

Traefik uses routers to match incoming requests:

```yaml
# Django - matches all requests to example.com
- "traefik.http.routers.django.rule=Host(`example.com`)"

# WebSocket - matches /ws/ paths with higher priority
- "traefik.http.routers.websocket.rule=Host(`example.com`) && PathPrefix(`/ws/`)"
- "traefik.http.routers.websocket.priority=100"
```

### Priority

Set higher priority for WebSocket routes to match before Django:

```yaml
- "traefik.http.routers.websocket.priority=100"
```

### TLS/SSL

Automatic HTTPS with Let's Encrypt:

```yaml
- "traefik.http.routers.websocket.tls.certresolver=letsencrypt"
```

## Complete Example

```yaml
# docker-compose.prod.yml
services:
  traefik:
    image: traefik:v3.6
    restart: always
    command:
      - "--log.level=INFO"
      - "--accesslog=true"
      - "--api.dashboard=true"
      - "--providers.docker=true"
      - "--providers.docker.exposedbydefault=false"
      - "--entrypoints.web.address=:80"
      - "--entrypoints.websecure.address=:443"
      - "--entrypoints.websecure.http.tls=true"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge=true"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge.entrypoint=web"
      - "--certificatesresolvers.letsencrypt.acme.email=${ACME_EMAIL}"
      - "--certificatesresolvers.letsencrypt.acme.storage=/letsencrypt/acme.json"
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - letsencrypt:/letsencrypt
    networks:
      - web
      - internal

  django:
    image: myproject:latest
    restart: always
    command: gunicorn myproject.wsgi:application --bind 0.0.0.0:8000 --workers 4
    environment:
      - DJANGO_SETTINGS_MODULE=myproject.settings.production
      - DATABASE_URL=${DATABASE_URL}
      - REDIS_URL=${REDIS_URL}
      - SECRET_KEY=${SECRET_KEY}
    depends_on:
      - redis
      - db
    networks:
      - web
      - internal
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.django.rule=Host(`${DOMAIN}`)"
      - "traefik.http.routers.django.entrypoints=websecure"
      - "traefik.http.routers.django.tls.certresolver=letsencrypt"
      - "traefik.http.services.django.loadbalancer.server.port=8000"
      # HTTP to HTTPS redirect
      - "traefik.http.routers.django-http.rule=Host(`${DOMAIN}`)"
      - "traefik.http.routers.django-http.entrypoints=web"
      - "traefik.http.routers.django-http.middlewares=https-redirect"
      - "traefik.http.middlewares.https-redirect.redirectscheme.scheme=https"
      - "traefik.http.middlewares.https-redirect.redirectscheme.permanent=true"

  websocket:
    image: myproject:latest
    restart: always
    command: python manage.py runwebsockets --log-level info
    environment:
      - DJANGO_SETTINGS_MODULE=myproject.settings.production
      - DATABASE_URL=${DATABASE_URL}
      - REDIS_URL=${REDIS_URL}
      - SECRET_KEY=${SECRET_KEY}
    depends_on:
      - redis
      - db
    networks:
      - web
      - internal
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.websocket.rule=Host(`${DOMAIN}`) && PathPrefix(`/ws/`)"
      - "traefik.http.routers.websocket.entrypoints=websecure"
      - "traefik.http.routers.websocket.tls.certresolver=letsencrypt"
      - "traefik.http.routers.websocket.priority=100"
      - "traefik.http.services.websocket.loadbalancer.server.port=46290"
      # Rate limiting
      - "traefik.http.middlewares.ws-ratelimit.ratelimit.average=10"
      - "traefik.http.middlewares.ws-ratelimit.ratelimit.burst=50"
      - "traefik.http.routers.websocket.middlewares=ws-ratelimit"

  redis:
    image: redis:latest
    restart: always
    networks:
      - internal
    volumes:
      - redis_data:/data

  db:
    image: postgres:latest
    restart: always
    environment:
      - POSTGRES_USER=${DB_USER}
      - POSTGRES_PASSWORD=${DB_PASSWORD}
      - POSTGRES_DB=${DB_NAME}
    networks:
      - internal
    volumes:
      - postgres_data:/var/lib/postgresql/data

networks:
  web:
    external: true
  internal:

volumes:
  letsencrypt:
  redis_data:
  postgres_data:
```
