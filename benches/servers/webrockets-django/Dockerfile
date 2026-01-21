FROM python:3.13-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv

# Copy wheel and install with django
COPY wheels/*.whl /tmp/
RUN uv pip install --system /tmp/*.whl django && rm /tmp/*.whl

COPY servers/webrockets-django/settings.py servers/webrockets-django/models.py servers/webrockets-django/server.py ./

EXPOSE 6970

CMD ["python", "server.py"]
