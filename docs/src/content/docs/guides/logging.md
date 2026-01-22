---
title: Logging
description: Configure logging for webrockets.
---

webrockets uses Python's standard logging module through `pyo3_log`, which bridges Rust's log crate to Python.

## Logger Names

| Logger | Description |
|--------|-------------|
| `webrockets` | Root logger for all webrockets logs |
| `webrockets.server` | Server startup, shutdown, and connection errors |
| `webrockets.broker.redis` | Redis broker connection and message logs |
| `webrockets.broker.amqp` | RabbitMQ broker connection and message logs |

## Django Configuration

### Management Command

Use the `--log-level` flag:

```bash
python manage.py runwebsockets --log-level info
```

Available levels: `debug`, `info`, `warning`, `error`, `critical`

### Django Settings

Configure logging in your Django settings:

```python
# settings.py
LOGGING = {
    "version": 1,
    "disable_existing_loggers": False,
    "formatters": {
        "verbose": {
            "format": "{asctime} {levelname} {name}: {message}",
            "style": "{",
        },
    },
    "handlers": {
        "console": {
            "class": "logging.StreamHandler",
            "formatter": "verbose",
        },
    },
    "loggers": {
        "webrockets": {
            "handlers": ["console"],
            "level": "INFO",
        },
        # More verbose logging for the broker
        "webrockets.broker": {
            "handlers": ["console"],
            "level": "DEBUG",
        },
    },
}
```

## Standalone Python

Configure logging before starting the server:

```python
import logging
from webrockets import WebsocketServer

# Simple configuration
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s %(levelname)s %(name)s: %(message)s"
)

# Or configure just webrockets
logger = logging.getLogger("webrockets")
logger.setLevel(logging.INFO)
handler = logging.StreamHandler()
handler.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(name)s: %(message)s"))
logger.addHandler(handler)

server = WebsocketServer()
# ... configure routes ...
server.start()
```

## structlog Integration

webrockets works with structlog since it uses standard Python logging:

```python
import logging
import structlog

structlog.configure(
    processors=[
        structlog.stdlib.filter_by_level,
        structlog.stdlib.add_logger_name,
        structlog.stdlib.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.JSONRenderer(),
    ],
    wrapper_class=structlog.stdlib.BoundLogger,
    context_class=dict,
    logger_factory=structlog.stdlib.LoggerFactory(),
)

# Configure webrockets to use structlog's logging
logging.getLogger("webrockets").setLevel(logging.INFO)
```

## Log Output Examples

### Server Startup

```
2024-01-15 10:30:00 INFO webrockets.server: Starting WebSocket server on 0.0.0.0:46290
```

### Connection Events

```
2024-01-15 10:30:05 DEBUG webrockets.server: Client connected: /ws/chat/
2024-01-15 10:30:10 DEBUG webrockets.server: Client disconnected: /ws/chat/ (code: 1000)
```

### Broker Events

```
2024-01-15 10:30:00 INFO webrockets.broker.redis: Connected to redis://localhost:6379
2024-01-15 10:30:05 DEBUG webrockets.broker.redis: Published to channel: ws_broadcast
```

## Troubleshooting

### No logs appearing

Ensure you configure logging before starting the server:

```python
import logging
logging.basicConfig(level=logging.DEBUG)  # Must come before server.start()
server.start()
```

### Too verbose

Set a higher log level for specific loggers:

```python
logging.getLogger("webrockets.broker").setLevel(logging.WARNING)
```

### Duplicate logs

Prevent log propagation if you have multiple handlers:

```python
logger = logging.getLogger("webrockets")
logger.propagate = False
```
