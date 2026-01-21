FROM python:3.13-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv

# Copy wheel and install
COPY wheels/*.whl /tmp/
RUN uv pip install --system /tmp/*.whl && rm /tmp/*.whl

COPY servers/webrockets/server.py .

EXPOSE 6969

CMD ["python", "server.py"]
