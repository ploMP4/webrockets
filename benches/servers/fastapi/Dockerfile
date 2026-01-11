FROM python:3.13-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv

RUN uv pip install --system fastapi "uvicorn[standard]"

COPY servers/fastapi/server.py .

EXPOSE 1234

CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "1234"]
