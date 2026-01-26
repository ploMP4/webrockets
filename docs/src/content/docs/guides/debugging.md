---
title: Debugging
description: How to debug WebSocket callbacks using remote-pdb.
---

Debugging WebSocket callbacks requires special tools because the Rust server runs callbacks in a separate thread. Standard `pdb` and `breakpoint()` won't work due to signal handling limitations in non-main threads.

## The Problem

If you try to use the standard Python debugger:

```python
@route.receive
def receive(conn: Connection, data: str | bytes):
    breakpoint()  # This will fail!
    conn.send(data)
```

You'll get an error:

```
ValueError: signal only works in main thread of the main interpreter
```

## Solution: remote-pdb

Use `remote-pdb` which provides a network-based debugger that works from any thread.

### Installation

```bash
pip install remote-pdb
```

### Usage

Add a remote breakpoint in your callback:

```python
@route.receive
def receive(conn: Connection, data: str | bytes):
    from remote_pdb import RemotePdb
    RemotePdb("127.0.0.1", 4444).set_trace()
    conn.send(data)
```

Run your tests or server:

```bash
pytest tests/test_myapp.py
```

Connect from another terminal:

```bash
telnet 127.0.0.1 4444
# or
nc 127.0.0.1 4444
```

You'll get an interactive pdb prompt where you can inspect variables and step through code.

## Using PYTHONBREAKPOINT

Configure Python's built-in `breakpoint()` to use remote-pdb:

```bash
PYTHONBREAKPOINT=remote_pdb.set_trace \
REMOTE_PDB_HOST=127.0.0.1 \
REMOTE_PDB_PORT=4444 \
pytest tests/test_myapp.py
```

Now you can use `breakpoint()` normally in your callbacks:

```python
@route.receive
def receive(conn: Connection, data: str | bytes):
    breakpoint()  # Uses remote-pdb via environment variable
    conn.send(data)
```

## Alternative: debugpy

For IDE integration (VS Code, PyCharm), you can use `debugpy`, but it requires setup before the server starts:

```python
import debugpy

# Must be called BEFORE server starts, in main thread
debugpy.listen(("0.0.0.0", 5678))
debugpy.wait_for_client()  # Optional: wait for IDE to attach

# Now start your server/tests
```

Then attach your IDE debugger to port 5678. Breakpoints set via `debugpy.breakpoint()` will work in callbacks.
