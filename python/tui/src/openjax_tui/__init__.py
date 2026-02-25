"""OpenJax TUI - A modern terminal UI for OpenJax.

This package provides a Textual-based terminal user interface for the OpenJax
AI agent framework.
"""

from __future__ import annotations

import os

__version__ = "0.1.0"
__all__ = ["main", "OpenJaxApp"]


def main() -> None:
    """Entry point for the OpenJax TUI application."""
    from .app import OpenJaxApp
    from .logging_setup import get_logger, setup_logging

    logger = setup_logging()
    logger.info("openjax_tui starting")
    app = OpenJaxApp()
    mouse_enabled = _mouse_enabled_by_env()
    logger.info("openjax_tui mouse_enabled=%s", mouse_enabled)
    try:
        app.run(mouse=mouse_enabled)
    except Exception:
        get_logger().exception("openjax_tui fatal_error")
        raise
    finally:
        get_logger().info("openjax_tui exited")


def _mouse_enabled_by_env() -> bool:
    """Enable Textual mouse reporting only when explicitly requested."""
    raw = os.environ.get("OPENJAX_TUI_ENABLE_MOUSE", "").strip().lower()
    return raw in {"1", "true", "yes", "on"}
