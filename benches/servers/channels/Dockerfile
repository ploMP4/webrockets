FROM python:3.13-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv

RUN uv pip install --system django "channels[daphne]"

COPY servers/channels/settings.py servers/channels/server.py ./

EXPOSE 8000

CMD ["daphne", "-b", "0.0.0.0", "-p", "8000", "server:application"]
