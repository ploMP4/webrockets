"""Minimal Django settings for Bolt benchmark server."""

from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent

SECRET_KEY = "benchmark-secret-key-not-for-production"
DEBUG = False
ALLOWED_HOSTS = ["*"]

INSTALLED_APPS = [
    "django.contrib.contenttypes",
    "django_bolt",
]

DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.sqlite3",
        "NAME": "/tmp/bench_bolt.db",
    }
}

DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"
