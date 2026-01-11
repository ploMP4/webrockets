"""Django models for benchmark database operations."""

from django.db import models


class Message(models.Model):
    """Simple message model for db benchmarks."""

    content = models.TextField()
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        app_label = "bench"
        db_table = "bench_messages"
