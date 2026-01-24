# WebSocket Benchmark Suite

Benchmark suite for comparing WebSocket server implementations in Python.

The benchmark runner (`run.js`) and load test client (`load_test.c`) are adapted from the [fastwebsockets benchmarks](https://github.com/denoland/fastwebsockets/tree/main/benches).

## Servers

| Server | Port | Description |
|--------|------|-------------|
| webrockets | 6969 | Rust-powered WebSocket server (standalone) |
| webrockets-django | 6970 | webrockets with Django ORM integration |
| django-channels | 8000 | Django Channels with Daphne ASGI |
| fastapi | 1234 | FastAPI with uvicorn |
| django-bolt | 1235 | Django-Bolt async framework |
| python-websockets | 4200 | Pure Python websockets library |

## Quick Start

### 1. Build webrockets wheel

```bash
# From repo root
maturin build --release

# Copy wheel to benches
cp target/wheels/webrockets-*.whl benches/wheels/
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
deno run --allow-all run.js
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

## Directory Structure

```
benches/
├── servers/
│   ├── webrockets/          # Standalone webrockets server
│   ├── webrockets-django/   # webrockets with Django ORM
│   ├── channels/        # Django Channels
│   ├── fastapi/         # FastAPI
│   ├── bolt/            # Django-Bolt
│   ├── websockets/      # Pure Python websockets
├── wheels/              # Local webrockets wheel (not committed)
│   └── webrockets-*.whl
├── docker-compose.yml   # All servers
├── load_test.c          # C benchmark client
├── load_test            # Compiled binary
├── run.js               # Deno benchmark runner
└── *.svg                # Generated charts
```
