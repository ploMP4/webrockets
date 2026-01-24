---
title: Benchmarks
description: Performance comparison of webrockets against other Python WebSocket implementations.
---

This page presents benchmark results comparing webrockets against other popular Python WebSocket server implementations.

## Test Environment

All benchmarks were run using Docker containers on the same host machine. The benchmark client is written in C using [uSockets](https://github.com/uNetworking/uSockets) for maximum client-side performance, ensuring the bottleneck is always the server.

### Servers Tested

| Server | Description |
|--------|-------------|
| **webrockets** | Rust-powered WebSocket server (this library) |
| **FastAPI** | FastAPI with uvicorn |
| **python-websockets** | Pure Python websockets library |
| **Django Channels** | Django Channels with Daphne ASGI server |

## Standard Load (100 connections, 20 bytes)

![100 connections, 20 byte messages](/benchmarks/100-20-chart.svg)

**Key findings:**
- webrockets achieves ~84,000 msg/sec
- Django Channels with Daphne achieves ~6,000 msg/sec
- **webrockets is approximately 14x faster** than Django Channels in this scenario

## Few Connections, Larger Messages (10 connections, 1KB)

![10 connections, 1KB messages](/benchmarks/10-1024-chart.svg)

webrockets maintains its performance advantage across different payload sizes.

## High Concurrency (500 connections, 16KB)

![500 connections, 16KB messages](/benchmarks/500-16384-chart.svg)

## Intensive Load (10,000 connections, 1KB)

### Throughput

![10000 connections, 1KB messages](/benchmarks/10000-1024-chart.svg)

**Key findings:**
- webrockets handles 10,000 concurrent connections with ~64,000 msg/sec throughput
- Django Channels with Daphne achieves ~37,000 msg/sec
- python-websockets achieves ~32,000 msg/sec
- FastAPI with uvicorn achieves ~22,000 msg/sec

### Connection Time

While all servers can handle 10,000 connections, there's a significant difference in how quickly all clients can connect.

![Connection time for 10000 clients](/benchmarks/10000-1024-connect-chart.svg)

**Key findings:**
- webrockets connects all 10,000 clients in ~860ms
- Django Channels takes ~133,000ms (over 2 minutes) to accept all connections
- **webrockets accepts connections ~155x faster**

This difference is critical for applications where clients may connect in bursts, such as after a server restart or during traffic spikes.

## Why is webrockets faster?

1. **Rust core**: The WebSocket protocol handling, message parsing, and I/O are implemented in Rust using [axum](https://github.com/tokio-rs/axum) and [fastwebsockets](https://github.com/denoland/fastwebsockets)

2. **Multi-threaded**: Unlike Python’s asyncio event loop (which runs in a single thread by default), webrockets uses Tokio’s multi-threaded runtime to distribute connections across multiple CPU cores.

3. **Zero-copy where possible**: The Rust implementation minimizes memory allocations and copies

## Running the Benchmarks

You can reproduce these benchmarks yourself. See the [benchmark suite README](https://github.com/ploMP4/webrockets/tree/main/benches) for instructions.

The benchmark runner and load test client are adapted from the [fastwebsockets benchmarks](https://github.com/denoland/fastwebsockets/tree/main/benches).
