# WebSocket Benchmark Suite

Benchmark suite for comparing WebSocket server implementations in Python.

## Servers

| Server | Port | Description |
|--------|------|-------------|
| pywsrs | 6969 | Rust-powered WebSocket server (standalone) |
| pywsrs-django | 6970 | pywsrs with Django ORM integration |
| django-channels | 8000 | Django Channels with Daphne ASGI |
| fastapi | 1234 | FastAPI with uvicorn |
| django-bolt | 1235 | Django-Bolt async framework |
| python-websockets | 4200 | Pure Python websockets library |

## Routes

Each server exposes three endpoints for different benchmark scenarios:

- `/echo` - Simple echo (baseline throughput)
- `/db` - SQLite insert + query (database overhead)
- `/compute` - 1000x SHA256 iterations (CPU-bound)

## Quick Start

### 1. Build pywsrs wheel

```bash
# From repo root
maturin build --release

# Copy wheel to benches
cp target/wheels/pywsrs-*.whl benches/wheels/
```

### 2. Build the benchmark client

```bash
cd benches
gcc load_test.c -lusockets -o load_test
```

Requires [uSockets](https://github.com/uNetworking/uSockets) library.

### 3. Start all servers

```bash
docker compose up -d --build
```

### 4. Run benchmarks

```bash
# Echo benchmark (default)
deno run --allow-all run.js

# Database benchmark
deno run --allow-all run.js db

# Compute benchmark
deno run --allow-all run.js compute
```

## Manual Testing

Test individual servers with websocat:

```bash
# pywsrs
echo "hello" | websocat ws://localhost:6969/echo

# pywsrs-django
echo "hello" | websocat ws://localhost:6970/echo

# django-channels
echo "hello" | websocat ws://localhost:8000/echo

# fastapi
echo "hello" | websocat ws://localhost:1234/echo

# django-bolt
echo "hello" | websocat ws://localhost:1235/echo

# python-websockets
echo "hello" | websocat ws://localhost:4200/echo
```

## Test Cases

The benchmark runs these scenarios:

| Connections | Payload (bytes) | Description |
|-------------|-----------------|-------------|
| 100 | 20 | Many connections, tiny messages |
| 10 | 1024 | Few connections, 1KB messages |
| 10 | 16384 | Few connections, 16KB messages |
| 200 | 16384 | Medium connections, 16KB messages |
| 500 | 16384 | High connections, 16KB messages |
| 10000 | 1024 | Extreme connections, 1KB messages |

## Output

Results are saved as SVG bar charts:
- `echo-{conn}-{bytes}-chart.svg`
- `db-{conn}-{bytes}-chart.svg`
- `compute-{conn}-{bytes}-chart.svg`

## Adding New Routes

To add a new route type (e.g., `/auth`):

1. Add the handler to each server in `servers/*/server.py`
2. Add the route name to `validRoutes` in `run.js`
3. Rebuild and restart: `docker compose up -d --build`

## Directory Structure

```
benches/
├── servers/
│   ├── pywsrs/          # Standalone pywsrs server
│   ├── pywsrs-django/   # pywsrs with Django ORM
│   ├── channels/        # Django Channels
│   ├── fastapi/         # FastAPI
│   ├── bolt/            # Django-Bolt
│   └── websockets/      # Pure Python websockets
├── wheels/              # Local pywsrs wheel (not committed)
│   └── pywsrs-*.whl
├── docker-compose.yml   # All servers
├── load_test.c          # C benchmark client
├── load_test            # Compiled binary
├── run.js               # Deno benchmark runner
└── *.svg                # Generated charts
```
