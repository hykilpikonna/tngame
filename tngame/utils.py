import logging


def setup_logger(debug: bool):
    # Try to use rich for pretty printing
    try:
        from rich.logging import RichHandler
        handler = RichHandler(rich_tracebacks=True)

        from rich.traceback import install
        install(show_locals=True)
    except ImportError:
        handler = logging.StreamHandler()

    # Initialize debug logger
    logging.basicConfig(
        level="NOTSET" if debug else "INFO",
        format="%(message)s",
        datefmt="[%X]",
        handlers=[handler]
    )

    return logging.getLogger("a2")
