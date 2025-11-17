import django_wsrs

from django.core.management.base import BaseCommand


class Command(BaseCommand):
    def handle(self, *args, **options):
        django_wsrs.run_server()
