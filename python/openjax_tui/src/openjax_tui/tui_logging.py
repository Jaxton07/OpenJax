from __future__ import annotations

import contextlib
import logging
from logging.handlers import RotatingFileHandler
import os

_TUI_LOGGER_NAME = "openjax_tui"
_TUI_LOG_FILENAME = "openjax_tui.log"
_TUI_LOG_MAX_BYTES_DEFAULT = 2 * 1024 * 1024
_TUI_LOG_BACKUP_COUNT = 5
_tui_logger: logging.Logger | None = None


def _tui_debug_enabled() -> bool:
    flag = os.environ.get("OPENJAX_TUI_DEBUG", "")
    return flag.lower() in {"1", "true", "yes", "on"}


def _tui_debug(message: str) -> None:
    if not _tui_debug_enabled():
        return
    logger = _tui_logger
    if logger is not None:
        logger.debug(message)


def _tui_log_info(message: str) -> None:
    logger = _tui_logger
    if logger is not None:
        logger.info(message)


def _setup_tui_logger() -> logging.Logger | None:
    global _tui_logger
    if _tui_logger is not None:
        return _tui_logger

    log_dir = os.environ.get("OPENJAX_TUI_LOG_DIR", os.path.join(".openjax", "logs"))
    max_bytes = _parse_log_max_bytes(
        os.environ.get("OPENJAX_TUI_LOG_MAX_BYTES", ""),
        _TUI_LOG_MAX_BYTES_DEFAULT,
    )
    log_path = os.path.join(log_dir, _TUI_LOG_FILENAME)

    with contextlib.suppress(OSError):
        os.makedirs(log_dir, exist_ok=True)

    logger = logging.getLogger(_TUI_LOGGER_NAME)
    logger.setLevel(logging.DEBUG)
    logger.propagate = False
    for handler in list(logger.handlers):
        logger.removeHandler(handler)
        with contextlib.suppress(Exception):
            handler.close()

    try:
        handler = RotatingFileHandler(
            log_path,
            maxBytes=max_bytes,
            backupCount=_TUI_LOG_BACKUP_COUNT,
            encoding="utf-8",
        )
    except OSError as err:
        print(f"[warn] failed to initialize tui logger at {log_path}: {err}", file=os.sys.stderr)
        _tui_logger = None
        return None

    handler.setLevel(logging.DEBUG)
    handler.setFormatter(
        logging.Formatter("%(asctime)s %(levelname)s %(name)s %(message)s")
    )
    logger.addHandler(handler)
    logger.info(
        "tui logger initialized path=%s max_bytes=%s backups=%s",
        log_path,
        max_bytes,
        _TUI_LOG_BACKUP_COUNT,
    )
    _tui_logger = logger
    return logger


def _parse_log_max_bytes(raw: str, fallback: int) -> int:
    with contextlib.suppress(ValueError):
        value = int(raw.strip())
        if value > 0:
            return value
    return fallback


def _reset_tui_logger_for_tests() -> None:
    global _tui_logger
    logger = _tui_logger
    if logger is None:
        return
    for handler in list(logger.handlers):
        logger.removeHandler(handler)
        with contextlib.suppress(Exception):
            handler.close()
    _tui_logger = None
