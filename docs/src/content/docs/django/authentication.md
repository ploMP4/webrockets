---
title: Authentication
description: Authenticate WebSocket connections in Django.
---

webrockets provides authentication classes for validating WebSocket connections, following patterns similar to Django REST Framework. Authentication happens during the WebSocket handshake.

## Available Authentication Classes

```python
from webrockets.django.auth import (
    SessionAuthentication,           # Django session-based auth
    CookieTokenAuthentication,       # Token from cookie
    HeaderTokenAuthentication,       # Token from header
    QueryStringTokenAuthentication,  # Token from URL query parameter
)
```

## Session Authentication

Uses Django's session framework. Ideal for browser clients with active Django sessions:

```python
from webrockets.django import server
from webrockets.django.auth import SessionAuthentication

chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[SessionAuthentication()]
)

@chat.receive
def on_message(conn, data):
    # conn.user is the Django user from the session
    print(f"Message from {conn.user.username}")
```

### Custom Session Cookie Name

```python
auth = SessionAuthentication(session_cookie_name="my_session")
```

## Token Authentication

For JWT or custom tokens, subclass the token authentication classes.

### Cookie Token Authentication

Best for browser clients using JWT stored in cookies:

```python
import jwt
from django.conf import settings
from django.contrib.auth import get_user_model
from webrockets.django.auth import CookieTokenAuthentication, AuthenticationFailed

class JWTCookieAuth(CookieTokenAuthentication):
    cookie_name = "jwt_token"

    def validate_token(self, token):
        try:
            payload = jwt.decode(
                token,
                settings.SECRET_KEY,
                algorithms=["HS256"]
            )
            User = get_user_model()
            return User.objects.get(pk=payload["user_id"])
        except jwt.ExpiredSignatureError:
            raise AuthenticationFailed("Token expired", close_code=4001)
        except (jwt.InvalidTokenError, User.DoesNotExist):
            raise AuthenticationFailed("Invalid token", close_code=4003)
```

### Header Token Authentication

For non-browser clients (mobile apps, backend services):

```python
from webrockets.django.auth import HeaderTokenAuthentication

class BearerTokenAuth(HeaderTokenAuthentication):
    header_name = "authorization"
    keyword = "Bearer"

    def validate_token(self, token):
        return verify_and_get_user(token)
```

Expected header format: `Authorization: Bearer <token>`

:::note
Browser WebSocket connections cannot set custom headers during the handshake. Use this for non-browser clients or combine with other authentication methods.
:::

### Query String Token Authentication

Useful for browser clients that cannot set headers or cookies:

```python
from webrockets.django.auth import QueryStringTokenAuthentication

class TokenQueryAuth(QueryStringTokenAuthentication):
    query_param = "token"

    def validate_token(self, token):
        return verify_and_get_user(token)
```

Connection URL: `ws://example.com/ws/chat/?token=xyz123`

:::caution
Query string tokens may appear in server logs. Use short-lived tokens for this authentication method.
:::

## Multiple Authentication Classes

Combine multiple authentication methods. They are tried in order until one succeeds:

```python
from webrockets.django import server
from webrockets.django.auth import SessionAuthentication

class TokenAuth(QueryStringTokenAuthentication):
    def validate_token(self, token):
        return verify_token(token)

chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[
        SessionAuthentication(),  # Try session first
        TokenAuth(),              # Fall back to token
    ]
)
```

### Authentication Flow

1. First authenticator's `authenticate()` is called
2. If it returns a user, authentication succeeds
3. If it returns `None`, try the next authenticator
4. If it raises `AuthenticationFailed`, connection is rejected
5. If all return `None`, `conn.user` is `None`

## Anonymous Access

To allow unauthenticated connections, don't specify any authentication classes:

```python
# No authentication required
public_chat = server.create_route("ws/public/", "public")

@public_chat.receive
def on_message(conn, data):
    # conn.user is None
    if conn.user:
        name = conn.user.username
    else:
        name = "Anonymous"
    conn.broadcast(["public"], f"{name}: {data}")
```

## Custom Authentication

Create custom authentication by subclassing `BaseAuthentication`:

```python
from webrockets import BaseAuthentication, IncomingConnection
from webrockets.auth import AuthenticationFailed

class APIKeyAuthentication(BaseAuthentication):
    def authenticate(self, conn: IncomingConnection):
        api_key = conn.get_header("x-api-key")
        if not api_key:
            return None  # Skip, try next authenticator

        user = validate_api_key(api_key)
        if not user:
            raise AuthenticationFailed("Invalid API key", close_code=4003)

        return user
```

## AuthenticationFailed Exception

Raise `AuthenticationFailed` to reject a connection:

```python
from webrockets.auth import AuthenticationFailed

raise AuthenticationFailed(
    detail="Token expired",  # Error message
    close_code=4001          # WebSocket close code
)
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `detail` | `str` | `"Authentication failed"` | Error message |
| `close_code` | `int` | `4003` | WebSocket close code |

## Complete Example

```python
import jwt
from django.conf import settings
from django.contrib.auth import get_user_model
from webrockets.django import server
from webrockets.django.auth import (
    SessionAuthentication,
    CookieTokenAuthentication,
    QueryStringTokenAuthentication,
    AuthenticationFailed,
)

User = get_user_model()

class JWTCookieAuth(CookieTokenAuthentication):
    cookie_name = "jwt"

    def validate_token(self, token):
        try:
            payload = jwt.decode(token, settings.SECRET_KEY, algorithms=["HS256"])
            return User.objects.get(pk=payload["user_id"])
        except Exception:
            raise AuthenticationFailed("Invalid JWT")

class JWTQueryAuth(QueryStringTokenAuthentication):
    query_param = "token"

    def validate_token(self, token):
        try:
            payload = jwt.decode(token, settings.SECRET_KEY, algorithms=["HS256"])
            return User.objects.get(pk=payload["user_id"])
        except Exception:
            raise AuthenticationFailed("Invalid token")

# Support multiple auth methods
chat = server.create_route(
    "ws/chat/",
    "chat",
    authentication_classes=[
        SessionAuthentication(),  # Browser with session
        JWTCookieAuth(),          # Browser with JWT cookie
        JWTQueryAuth(),           # Any client with token in URL
    ]
)

@chat.receive
def on_message(conn, data):
    if conn.user:
        conn.broadcast(["chat"], f"{conn.user.username}: {data}")
    else:
        conn.send("Anonymous users cannot send messages")
```

## Next Steps

- [Broadcasting](/django/broadcasting/) - Send messages from Django to clients
- [Deployment](/django/deployment/overview/) - Deploy to production
