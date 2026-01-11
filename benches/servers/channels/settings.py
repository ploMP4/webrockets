"""Minimal Django settings for Channels benchmark server."""

from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent

SECRET_KEY = "benchmark-secret-key-not-for-production"
DEBUG = False
ALLOWED_HOSTS = ["*"]

INSTALLED_APPS = [
    "daphne",
    "django.contrib.contenttypes",
    "channels",
]

DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.sqlite3",
        "NAME": "/tmp/bench_channels.db",
    }
}

DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"

ASGI_APPLICATION = "server.application"
