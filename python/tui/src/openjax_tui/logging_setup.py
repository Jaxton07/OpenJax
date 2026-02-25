"""Logging setup for OpenJax Textual TUI."""

from __future__ import annotations

import contextlib
import logging
import os
import sys
from logging.handlers import RotatingFileHandler
from pathlib import Path
from types import TracebackType

LOGGER_NAME = "openjax_tui"
LOG_FILENAME = "openjax_tui.log"
DEFAULT_MAX_BYTES = 2 * 1024 * 1024
BACKUP_COUNT = 5

_logger: logging.Logger | None = None
_hooks_installed = False


def setup_logging() -> logging.Logger:
    """Configure and return the shared TUI logger."""
    global _logger

    if _logger is not None:
        return _logger

    log_dir = Path(os.environ.get("OPENJAX_TUI_LOG_DIR", ".openjax/logs"))
    max_bytes = _parse_max_bytes(
        os.environ.get("OPENJAX_TUI_LOG_MAX_BYTES", ""),
        DEFAULT_MAX_BYTES,
    )
    log_path = log_dir / LOG_FILENAME
    level = logging.DEBUG if _debug_enabled() else logging.INFO

    with contextlib.suppress(OSError):
        log_dir.mkdir(parents=True, exist_ok=True)

    logger = logging.getLogger(LOGGER_NAME)
    logger.setLevel(level)
    logger.propagate = False

    for handler in list(logger.handlers):
        logger.removeHandler(handler)
        with contextlib.suppress(Exception):
            handler.close()

    try:
        handler = RotatingFileHandler(
            log_path,
            maxBytes=max_bytes,
            backupCount=BACKUP_COUNT,
            encoding="utf-8",
        )
    except OSError as err:
        print(f"[warn] failed to initialize tui logger at {log_path}: {err}", file=sys.stderr)
        return logger

    handler.setLevel(level)
    handler.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(name)s %(message)s"))
    logger.addHandler(handler)
    logger.info(
        "logging initialized path=%s level=%s max_bytes=%s backups=%s",
        log_path,
        logging.getLevelName(level),
        max_bytes,
        BACKUP_COUNT,
    )

    _logger = logger
    install_exception_hooks(logger)
    return logger


def get_logger() -> logging.Logger:
    """Get the configured logger, initializing it if needed."""
    return setup_logging()


def install_exception_hooks(logger: logging.Logger | None = None) -> None:
    """Install hooks to capture uncaught exceptions into the log."""
    global _hooks_installed

    if _hooks_installed:
        return

    active_logger = logger or setup_logging()
    original_excepthook = sys.excepthook

    def _log_excepthook(
        exc_type: type[BaseException],
        exc_value: BaseException,
        exc_traceback: TracebackType | None,
    ) -> None:
        active_logger.exception("uncaught_exception", exc_info=(exc_type, exc_value, exc_traceback))
        original_excepthook(exc_type, exc_value, exc_traceback)

    sys.excepthook = _log_excepthook
    _hooks_installed = True


def _parse_max_bytes(raw: str, fallback: int) -> int:
    with contextlib.suppress(ValueError):
        value = int(raw.strip())
        if value > 0:
            return value
    return fallback


def _debug_enabled() -> bool:
    value = os.environ.get("OPENJAX_TUI_DEBUG", "")
    return value.lower() in {"1", "true", "yes", "on"}


def _reset_logging_for_tests() -> None:
    """Reset global logger state for unit tests."""
    global _logger
    global _hooks_installed

    logger = _logger
    if logger is not None:
        for handler in list(logger.handlers):
            logger.removeHandler(handler)
            with contextlib.suppress(Exception):
                handler.close()
    _logger = None
    _hooks_installed = False
