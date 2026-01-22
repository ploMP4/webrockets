---
title: Authentication
description: Implement custom authentication for WebSocket connections.
---

webrockets provides a flexible authentication system based on the `BaseAuthentication` class. This guide explains how authentication works and how to implement custom authentication for your WebSocket connections.

## How Authentication Works

Authentication in webrockets happens during the WebSocket handshake, before the connection is fully established. When a client connects:

1. Each authentication class is tried in order
2. If one returns a user object, authentication succeeds
3. If one raises `AuthenticationFailed`, the connection is rejected
4. If all return `None`, the connection proceeds with `conn.user = None`

```
Client connects
       │
       ▼
┌─────────────────┐
│ Auth Class 1    │──── returns user ────▶ Success (conn.user set)
└────────┬────────┘
         │ returns None
         ▼
┌─────────────────┐
│ Auth Class 2    │──── returns user ────▶ Success (conn.user set)
└────────┬────────┘
         │ returns None
         ▼
┌─────────────────┐
│ Auth Class 3    │──── raises error ────▶ Connection Rejected
└────────┬────────┘
         │ returns None
         ▼
    conn.user = None
    (anonymous access)
```

## BaseAuthentication

All authentication classes inherit from `BaseAuthentication`:

```python
from abc import ABC, abstractmethod
from webrockets import IncomingConnection

class BaseAuthentication(ABC):
    @abstractmethod
    def authenticate(self, conn: IncomingConnection) -> Any | None:
        """
        Authenticate the incoming connection.

        Args:
            conn: The incoming WebSocket connection with request data.

        Returns:
            A user object if authentication succeeds.
            None to skip this authenticator and try the next one.

        Raises:
            AuthenticationFailed: To reject the connection.
        """
        pass
```

## IncomingConnection

The `conn` parameter provides access to request data:

```python
def authenticate(self, conn: IncomingConnection):
    # Get the request path
    path = conn.path  # "/ws/chat/"

    # Get query string
    query = conn.query_string  # "room=general&token=abc123"

    # Get a specific cookie
    token = conn.get_cookie("auth_token")

    # Get a specific header
    api_key = conn.get_header("x-api-key")

    # Get the Origin header
    origin = conn.get_header("origin")
```

## AuthenticationFailed

Raise `AuthenticationFailed` to reject a connection:

```python
from webrockets.auth import AuthenticationFailed

raise AuthenticationFailed(
    detail="Invalid token",  # Error message
    close_code=4003          # WebSocket close code
)
```

Common close codes:
- `4001` - Unauthorized (missing credentials)
- `4003` - Forbidden (invalid credentials)

## Implementing Custom Authentication

### Example: API Key Authentication

Authenticate using an API key from a header:

```python
from webrockets import BaseAuthentication, IncomingConnection
from webrockets.auth import AuthenticationFailed

class APIKeyAuthentication(BaseAuthentication):
    def authenticate(self, conn: IncomingConnection):
        # Get API key from header
        api_key = conn.get_header("x-api-key")

        if not api_key:
            # No API key provided - skip to next authenticator
            return None

        # Validate the API key
        user = self.get_user_for_api_key(api_key)

        if not user:
            # Invalid API key - reject connection
            raise AuthenticationFailed("Invalid API key", close_code=4003)

        return user

    def get_user_for_api_key(self, api_key: str):
        # Your validation logic here
        # Return user object or None
        pass
```

### Example: Query String Token

Authenticate using a token in the URL:

```python
from urllib.parse import parse_qs
from webrockets import BaseAuthentication, IncomingConnection
from webrockets.auth import AuthenticationFailed

class QueryTokenAuthentication(BaseAuthentication):
    def authenticate(self, conn: IncomingConnection):
        # Parse query string
        params = parse_qs(conn.query_string)
        token = params.get("token", [None])[0]

        if not token:
            return None

        # Validate token
        user = self.validate_token(token)

        if not user:
            raise AuthenticationFailed("Invalid token", close_code=4003)

        return user

    def validate_token(self, token: str):
        # Your token validation logic
        pass
```

Client connects with: `ws://example.com/ws/chat/?token=abc123`

### Example: JWT Authentication

Authenticate using a JWT token from a cookie:

```python
import jwt
from webrockets import BaseAuthentication, IncomingConnection
from webrockets.auth import AuthenticationFailed

class JWTAuthentication(BaseAuthentication):
    def __init__(self, secret_key: str, cookie_name: str = "jwt"):
        self.secret_key = secret_key
        self.cookie_name = cookie_name

    def authenticate(self, conn: IncomingConnection):
        token = conn.get_cookie(self.cookie_name)

        if not token:
            return None

        try:
            payload = jwt.decode(
                token,
                self.secret_key,
                algorithms=["HS256"]
            )
            return self.get_user(payload)

        except jwt.ExpiredSignatureError:
            raise AuthenticationFailed("Token expired", close_code=4001)

        except jwt.InvalidTokenError:
            raise AuthenticationFailed("Invalid token", close_code=4003)

    def get_user(self, payload: dict):
        # Return user object based on JWT payload
        # This is application-specific
        return {"id": payload["user_id"], "name": payload["name"]}
```

### Example: Origin-Based Authentication

Restrict connections to specific origins:

```python
from webrockets import BaseAuthentication, IncomingConnection
from webrockets.auth import AuthenticationFailed

class OriginAuthentication(BaseAuthentication):
    allowed_origins = [
        "https://example.com",
        "https://app.example.com",
    ]

    def authenticate(self, conn: IncomingConnection):
        origin = conn.get_header("origin")

        if origin and origin not in self.allowed_origins:
            raise AuthenticationFailed(
                f"Origin {origin} not allowed",
                close_code=4003
            )

        # Don't set a user, just validate origin
        return None
```

## Using Authentication Classes

### Standalone Python

```python
from webrockets import WebsocketServer

server = WebsocketServer(host="0.0.0.0", port=8080)

chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[
        JWTAuthentication(secret_key="your-secret"),
        APIKeyAuthentication(),
    ]
)

@chat.receive
def on_message(conn, data):
    if conn.user:
        print(f"Message from {conn.user['name']}: {data}")
    else:
        conn.send("Anonymous users cannot send messages")
        conn.close()

server.start()
```

### Django

Django provides pre-built authentication classes:

```python
from webrockets.django import server
from webrockets.django.auth import SessionAuthentication

chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[SessionAuthentication()]
)
```

See the [Django Authentication guide](/django/authentication/) for Django-specific classes.

## Best Practices

1. **Return `None` to skip** - If credentials aren't present, return `None` to try the next authenticator
2. **Raise `AuthenticationFailed` to reject** - If credentials are invalid, raise an exception
3. **Use appropriate close codes** - 4001 for missing auth, 4003 for invalid auth
4. **Validate origins** - Consider adding origin validation for browser clients
5. **Use short-lived tokens** - Especially for query string tokens that may be logged

## Next Steps

- [Django Authentication](/django/authentication/) - Django-specific authentication classes
- [Reference: Authentication](/reference/authentication/) - Full API reference
