import importlib
import logging

if importlib.util.find_spec("django") is None:
    raise ImportError(
        'Django is required to use runwebsockets command. Install with: pip install "pywsrs[django]"'
    )

from django.core.management.base import BaseCommand
from django.utils.module_loading import autodiscover_modules

from pywsrs.django import server

LOG_LEVELS = {
    "debug": logging.DEBUG,
    "info": logging.INFO,
    "warning": logging.WARNING,
    "error": logging.ERROR,
    "critical": logging.CRITICAL,
}


class Command(BaseCommand):
    help = "Start the pywsrs WebSocket server"

    def add_arguments(self, parser):
        parser.add_argument(
            "--log-level",
            type=str,
            choices=LOG_LEVELS.keys(),
            default=None,
            help="Set the log level for pywsrs (debug, info, warning, error, critical)",
        )

    def handle(self, *args, **options):
        if options["log_level"]:
            level = LOG_LEVELS[options["log_level"]]
            logger = logging.getLogger("pywsrs")
            logger.setLevel(level)
            if not logger.handlers:
                handler = logging.StreamHandler()
                handler.setFormatter(
                    logging.Formatter("%(asctime)s %(levelname)s %(name)s: %(message)s")
                )
                logger.addHandler(handler)
                logger.propagate = False

        autodiscover_modules("websockets", "sockets", "sse", "views")
        server.start()
