---
title: Introduction
description: Learn what webrockets is and why you should use it.
---

webrockets is a high-performance WebSocket server for Python with first-class Django support. The core server is implemented in Rust using PyO3, providing exceptional performance while maintaining a Pythonic API.

## Why webrockets?

### Performance

The WebSocket server is built with Rust using:
- **axum** - Fast HTTP framework for routing and WebSocket upgrades
- **fastwebsockets** - High-performance WebSocket protocol implementation
- **tokio** - Async runtime for efficient connection handling

This architecture allows webrockets to handle thousands of concurrent connections with minimal resource usage.

### Django Integration

webrockets provides seamless Django integration:
- Pre-configured server singleton that reads from Django settings
- Authentication classes following Django REST Framework patterns
- Management command for route autodiscovery and server startup
- Session-based authentication out of the box

### Developer Experience

- Simple decorator-based API for handling connections and messages
- Pattern matching for routing messages based on JSON fields
- Optional Pydantic validation for message schemas
- Support for both sync and async callbacks

## Next Steps

- [Quick Start](/guides/quick-start/) - Get up and running in 5 minutes
- [Django Integration](/guides/django/) - Set up webrockets with Django
- [Pattern Matching](/guides/pattern-matching/) - Route messages by JSON fields
