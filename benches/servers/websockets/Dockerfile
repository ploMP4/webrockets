FROM python:3.13-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv

RUN uv pip install --system websockets

COPY servers/websockets/server.py .

EXPOSE 4200

CMD ["python", "server.py"]
