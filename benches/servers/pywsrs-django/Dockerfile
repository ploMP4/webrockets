FROM python:3.13-slim

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv

# Copy wheel and install with django
COPY wheels/*.whl /tmp/
RUN uv pip install --system /tmp/*.whl django && rm /tmp/*.whl

COPY servers/pywsrs-django/settings.py servers/pywsrs-django/models.py servers/pywsrs-django/server.py ./

EXPOSE 6970

CMD ["python", "server.py"]
