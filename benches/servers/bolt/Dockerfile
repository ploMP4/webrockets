FROM python:3.13-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv

RUN uv pip install --system django django-bolt "uvicorn[standard]"

COPY servers/bolt/settings.py servers/bolt/server.py ./

EXPOSE 1235

CMD ["uvicorn", "server:api", "--host", "0.0.0.0", "--port", "1235"]
