"""Minimal Django settings for pywsrs benchmark server."""

from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent

SECRET_KEY = "benchmark-secret-key-not-for-production"
DEBUG = False
ALLOWED_HOSTS = ["*"]

INSTALLED_APPS = [
    "django.contrib.contenttypes",
    "pywsrs",
]

DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.sqlite3",
        "NAME": "/tmp/bench_django.db",
    }
}

DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"

WEBSOCKET_HOST = "0.0.0.0"
WEBSOCKET_PORT = 6970
