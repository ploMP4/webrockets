# Chat Example

A simple real-time chat application demonstrating WebSocket communication with a TUI client.

## Running

Start the server:

```bash
uv run server.py
```

Then open one or more clients in separate terminals:

```bash
uv run client.py
```

Each client will prompt for a username and room name. Users in the same room can send messages to each other in real-time.

## Controls

- Type your message and press **Enter** to send
- Press **Ctrl+C** to quit
