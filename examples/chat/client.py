# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "webrockets",
#   "rich",
# ]
# ///

import asyncio
import json
import sys
import termios
import tty
from datetime import datetime
from queue import Empty, Queue
from threading import Thread

from rich.console import Console
from rich.layout import Layout
from rich.live import Live
from rich.panel import Panel
from rich.text import Text
from webrockets.client import AsyncClient, ClientConfig, ConnectionClosed, aconnect


class ChatClient:
    def __init__(self, name: str, room: str):
        self.name = name
        self.room = room
        self.console = Console()
        self.messages: list[tuple[str, str, str]] = []
        self.input_buffer = ""
        self.running = True
        self.ws: AsyncClient | None = None
        self.input_queue: Queue[str | None] = Queue()

    def render_messages(self) -> Panel:
        text = Text()
        for timestamp, sender, message in self.messages[-20:]:
            text.append(f"[{timestamp}] ", style="dim")
            if sender == "system":
                text.append(f"{message}\n", style="yellow italic")
            elif sender == self.name:
                text.append(f"{sender}: ", style="bold green")
                text.append(f"{message}\n")
            else:
                text.append(f"{sender}: ", style="bold cyan")
                text.append(f"{message}\n")
        if not self.messages:
            text.append("No messages yet...", style="dim italic")
        return Panel(text, title=f"Room: {self.room}", border_style="blue")

    def render_input(self) -> Panel:
        return Panel(
            Text(f"> {self.input_buffer}█", style="white"),
            title="Message (Enter to send, Ctrl+C to quit)",
            border_style="green",
            height=3,
        )

    def render(self) -> Layout:
        layout = Layout()
        layout.split_column(
            Layout(name="messages", ratio=9),
            Layout(name="input", ratio=1, minimum_size=3),
        )
        layout["messages"].update(self.render_messages())
        layout["input"].update(self.render_input())
        return layout

    def add_message(self, sender: str, message: str):
        timestamp = datetime.now().strftime("%H:%M:%S")
        self.messages.append((timestamp, sender, message))

    def input_thread(self):
        """Read input in a separate thread to avoid blocking."""
        fd = sys.stdin.fileno()
        old_settings = termios.tcgetattr(fd)
        try:
            tty.setcbreak(fd)  # Use cbreak instead of raw - allows Ctrl+C
            while self.running:
                char = sys.stdin.read(1)
                if char == "\x03":  # Ctrl+C
                    self.input_queue.put(None)
                    break
                self.input_queue.put(char)
        finally:
            termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)

    async def process_input(self):
        """Process input from the queue."""
        while self.running:
            try:
                char = self.input_queue.get_nowait()
                if char is None:  # Ctrl+C signal
                    self.running = False
                    break
                if char in ("\r", "\n"):
                    if self.input_buffer.strip():
                        msg = self.input_buffer.strip()
                        if self.ws:
                            await self.ws.send(json.dumps({"type": "message", "msg": msg}))
                            self.add_message(self.name, msg)
                    self.input_buffer = ""
                elif char == "\x7f" or char == "\x08":  # Backspace
                    self.input_buffer = self.input_buffer[:-1]
                elif char.isprintable():
                    self.input_buffer += char
            except Empty:
                await asyncio.sleep(0.02)

    async def recv_messages(self):
        """Receive messages from websocket."""
        assert self.ws is not None
        try:
            while True:
                raw = await self.ws.recv()
                try:
                    data = json.loads(raw)
                    msg_type = data.get("type")
                    if msg_type == "message":
                        sender = data.get("user", "unknown")
                        self.add_message(sender, data.get("msg", ""))
                    elif msg_type == "join":
                        self.add_message("system", f"{data.get('user', 'Someone')} joined")
                    elif msg_type == "leave":
                        self.add_message("system", f"{data.get('user', 'Someone')} left")
                    else:
                        self.add_message("system", str(data))
                except json.JSONDecodeError:
                    self.add_message("system", str(raw))
        except ConnectionClosed:
            self.add_message("system", "Connection closed")
            self.running = False

    async def run(self):
        # Start input thread
        input_thread = Thread(target=self.input_thread, daemon=True)
        input_thread.start()

        try:
            async with aconnect(
                "ws://localhost:46290/chat", ClientConfig(extra_headers={"username": self.name})
            ) as ws:
                self.ws = ws
                await ws.send(json.dumps({"type": "join", "room": self.room}))
                self.add_message("system", f"Connected to room '{self.room}'")

                recv_task = asyncio.create_task(self.recv_messages())
                input_task = asyncio.create_task(self.process_input())

                with Live(
                    self.render(),
                    console=self.console,
                    refresh_per_second=30,
                    screen=True,
                ) as live:
                    while self.running:
                        live.update(self.render())
                        await asyncio.sleep(0.033)  # ~30fps

                recv_task.cancel()
                input_task.cancel()
                await asyncio.gather(recv_task, input_task, return_exceptions=True)

        except RuntimeError as e:
            self.console.print(f"[red]Could not connect to server: {e}[/red]")
        except Exception as e:
            self.console.print(f"[red]Error: {e}[/red]")


def main():
    console = Console()
    console.print("[bold blue]Chat Client[/bold blue]")

    name = console.input("[green]What's your name?[/green] ")
    room = console.input("[green]Room name?[/green] ")

    if not name.strip() or not room.strip():
        console.print("[red]Name and room are required[/red]")
        return

    client = ChatClient(name.strip(), room.strip())

    try:
        asyncio.run(client.run())
    except KeyboardInterrupt:
        pass

    console.print("\n[yellow]Goodbye![/yellow]")


if __name__ == "__main__":
    main()
