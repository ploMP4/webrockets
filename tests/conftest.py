import django
import pytest
from django.conf import settings
from django.contrib.auth import get_user_model


def pytest_configure():
    """Configure Django settings for tests."""
    if not settings.configured:
        settings.configure(
            DEBUG=True,
            DATABASES={
                "default": {
                    "ENGINE": "django.db.backends.sqlite3",
                    "NAME": ":memory:",
                }
            },
            INSTALLED_APPS=[
                "django.contrib.contenttypes",
                "django.contrib.auth",
                "django.contrib.sessions",
                "pywsrs",
            ],
            SECRET_KEY="test-secret-key-for-testing-only",
            SESSION_ENGINE="django.contrib.sessions.backends.db",
            SESSION_COOKIE_NAME="sessionid",
            USE_TZ=True,
        )
        django.setup()


@pytest.fixture
def user_model():
    """Return the User model."""
    return get_user_model()


@pytest.fixture
def create_user(user_model):
    """Factory fixture to create users."""

    def _create_user(username="testuser", password="testpass123", **kwargs):
        return user_model.objects.create_user(username=username, password=password, **kwargs)

    return _create_user


@pytest.fixture
def active_user(create_user):
    """Create an active user."""
    return create_user(username="activeuser", is_active=True)


@pytest.fixture
def inactive_user(create_user):
    """Create an inactive user."""
    return create_user(username="inactiveuser", is_active=False)


@pytest.fixture
def session_store():
    """Return the session store class."""
    from django.contrib.sessions.backends.db import SessionStore

    return SessionStore


@pytest.fixture
def create_session(session_store, active_user):
    """Factory fixture to create sessions."""

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
    """Factory fixture to create Connection objects."""
    from pywsrs import Connection

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
