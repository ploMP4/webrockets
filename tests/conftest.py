import tempfile
import threading
import time
from pathlib import Path

import django
import pytest
from django.conf import settings
from django.contrib.auth import get_user_model
from webrockets import WebsocketServer

# Use a temp file for SQLite so all threads can access it
_test_db_file = Path(tempfile.gettempdir()) / "webrockets_test.sqlite3"


class RunServer:
    def __init__(self, server: WebsocketServer) -> None:
        self._server = server

    def __enter__(self):
        self._thread = threading.Thread(target=self._server.start)
        self._thread.start()
        time.sleep(0.1)
        return self

    def __exit__(self, exc_type, exc, tb):
        self._server.stop()
        self._thread.join(timeout=5)
        if self._thread.is_alive():
            raise RuntimeError("WebSocket server did not shut down cleanly")


def pytest_configure():
    """Configure Django settings for tests."""
    if not settings.configured:
        settings.configure(
            DEBUG=True,
            DATABASES={
                "default": {
                    "ENGINE": "django.db.backends.sqlite3",
                    "NAME": str(_test_db_file),
                    "OPTIONS": {
                        "timeout": 20,
                    },
                }
            },
            INSTALLED_APPS=[
                "django.contrib.contenttypes",
                "django.contrib.auth",
                "django.contrib.sessions",
                "webrockets",
            ],
            SECRET_KEY="test-secret-key-for-testing-only",
            SESSION_ENGINE="django.contrib.sessions.backends.db",
            SESSION_COOKIE_NAME="sessionid",
            USE_TZ=True,
            CACHES={
                "default": {
                    "BACKEND": "django.core.cache.backends.locmem.LocMemCache",
                }
            },
        )
        django.setup()


def pytest_unconfigure():
    """Clean up test database file."""
    if _test_db_file.exists():
        _test_db_file.unlink()


@pytest.fixture(scope="session")
def django_db_setup(django_db_blocker):
    from django.core.management import call_command
    from django.db import connection

    with django_db_blocker.unblock():
        call_command("migrate", "--run-syncdb", verbosity=0)

        with connection.cursor() as cursor:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS test_messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    content TEXT NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)


@pytest.fixture
def user_model():
    """Return the User model."""
    return get_user_model()


@pytest.fixture
def create_user(user_model, django_db_blocker):
    """Factory fixture to create users."""

    def _create_user(username="testuser", password="testpass123", **kwargs):
        with django_db_blocker.unblock():
            return user_model.objects.create_user(username=username, password=password, **kwargs)

    return _create_user


@pytest.fixture
def active_user(create_user):
    return create_user(username="activeuser", is_active=True)


@pytest.fixture
def inactive_user(create_user):
    return create_user(username="inactiveuser", is_active=False)


@pytest.fixture
def session_store():
    from django.contrib.sessions.backends.db import SessionStore

    return SessionStore


@pytest.fixture
def create_session(session_store, active_user):
    def _create_session(user=None, expired=False):
        user = user or active_user
        session = session_store()
        session["_auth_user_id"] = str(user.pk)
        session["_auth_user_backend"] = "django.contrib.auth.backends.ModelBackend"

        # Add session auth hash if user has it
        if hasattr(user, "get_session_auth_hash"):
            session["_auth_user_hash"] = user.get_session_auth_hash()

        session.create()

        if expired:
            # Set expiry to the past
            session.set_expiry(-3600)  # Expired 1 hour ago
            session.save()

        return session

    return _create_session


@pytest.fixture
def websocket_scope():
    from webrockets import Connection

    def _create_scope(
        path="/ws/test/",
        headers=None,
        cookies=None,
        query_string="",
    ):
        return Connection(
            path=path,
            headers=headers or {},
            cookies=cookies or {},
            query_string=query_string,
        )

    return _create_scope


@pytest.fixture
def ws_server():
    yield WebsocketServer()
