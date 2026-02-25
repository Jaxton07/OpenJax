"""OpenJax TUI - A modern terminal UI for OpenJax.

This package provides a Textual-based terminal user interface for the OpenJax
AI agent framework.
"""

from __future__ import annotations

__version__ = "0.1.0"
__all__ = ["main", "OpenJaxApp"]


def main() -> None:
    """Entry point for the OpenJax TUI application."""
    from .app import OpenJaxApp

    app = OpenJaxApp()
    app.run()
