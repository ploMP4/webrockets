import asyncio
import tempfile
import threading
from collections.abc import Callable
from functools import wraps
from pathlib import Path
from typing import Any

import django
import pytest
from django.conf import settings
from django.contrib.auth import get_user_model
from webrockets import WebsocketRoute, WebsocketServer
from webrockets.test import runserver

# Use a temp file for SQLite so all threads can access it
_test_db_file = Path(tempfile.gettempdir()) / "webrockets_test.sqlite3"


class _ErrorCapture:
    def __init__(self):
        self._errors: list[BaseException] = []
        self._lock = threading.Lock()

    def wrap(self, func: Callable) -> Callable:
        @wraps(func)
        def sync_wrapper(*args, **kwargs):
            try:
                return func(*args, **kwargs)
            except BaseException as e:
                with self._lock:
                    self._errors.append(e)
                raise

        @wraps(func)
        async def async_wrapper(*args, **kwargs):
            try:
                return await func(*args, **kwargs)
            except BaseException as e:
                with self._lock:
                    self._errors.append(e)
                raise

        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        return sync_wrapper

    def check(self):
        with self._lock:
            if self._errors:
                raise self._errors[0]


class _TestRoute:
    def __init__(self, route: WebsocketRoute, error_capture: _ErrorCapture):
        self._route = route
        self._errors = error_capture

    def connect(self, *args, **kwargs) -> Any:
        original_decorator = self._route.connect(*args, **kwargs)

        def wrapper(func):
            return original_decorator(self._errors.wrap(func))

        return wrapper

    def receive(self, *args, **kwargs) -> Any:
        if args and callable(args[0]) and not kwargs:
            # Called as @route.receive without arguments
            func = args[0]
            return self._route.receive(self._errors.wrap(func))
        else:
            # Called as @route.receive(...) with arguments
            original_decorator = self._route.receive(*args, **kwargs)

            def wrapper(func):
                return original_decorator(self._errors.wrap(func))

            return wrapper

    def disconnect(self, *args, **kwargs) -> Any:
        func = args[0]
        return self._route.disconnect(self._errors.wrap(func))

    def __getattr__(self, name):
        return getattr(self._route, name)


class TestWebsocketServer:
    def __init__(self, server: WebsocketServer | None = None) -> None:
        if server:
            self._server = server
        else:
            self._server = WebsocketServer()
        self._errors = _ErrorCapture()

    def create_route(self, *args, **kwargs) -> Any:
        route = self._server.create_route(*args, **kwargs)
        return _TestRoute(route, self._errors)

    def check_errors(self):
        self._errors.check()

    def __getattr__(self, name):
        return getattr(self._server, name)


class runtestserver(runserver):
    def __init__(self, server: WebsocketServer) -> None:
        if isinstance(server, TestWebsocketServer):
            self._server = server._server
            self._test_server = server
        else:
            self._server = server
            self._test_server = None

    def __exit__(self, exc_type, exc, tb):
        super().__exit__(exc_type, exc, tb)
        if exc_type is None and self._test_server is not None:
            self._test_server.check_errors()


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
    yield TestWebsocketServer()
