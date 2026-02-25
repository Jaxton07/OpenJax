"""Tests for package initialization."""

from __future__ import annotations

import unittest
from unittest.mock import MagicMock, patch


class TestPackageInit(unittest.TestCase):
    """Test package initialization and basic functionality."""

    def test_package_importable(self) -> None:
        """Test that the package can be imported."""
        import openjax_tui

        self.assertTrue(hasattr(openjax_tui, "__version__"))
        self.assertTrue(hasattr(openjax_tui, "main"))

    def test_version_exists(self) -> None:
        """Test that version is defined."""
        from openjax_tui import __version__

        self.assertIsInstance(__version__, str)
        self.assertEqual(__version__, "0.1.0")

    def test_main_function_exists(self) -> None:
        """Test that main function exists and is callable."""
        from openjax_tui import main

        self.assertTrue(callable(main))

    def test_main_runs_app(self) -> None:
        """Test that main function runs the app."""
        from openjax_tui import main

        with (
            patch("openjax_tui.app.OpenJaxApp") as mock_app_class,
            patch("openjax_tui.logging_setup.setup_logging") as mock_setup_logging,
            patch("openjax_tui.logging_setup.get_logger") as mock_get_logger,
        ):
            mock_app = MagicMock()
            mock_app_class.return_value = mock_app
            mock_logger = MagicMock()
            mock_setup_logging.return_value = mock_logger
            mock_get_logger.return_value = mock_logger

            main()

            mock_app_class.assert_called_once()
            mock_app.run.assert_called_once()
            mock_setup_logging.assert_called_once()


class TestModuleEntryPoint(unittest.TestCase):
    """Test module entry point functionality."""

    def test_module_imports(self) -> None:
        """Test that the module can be imported."""
        import openjax_tui

        self.assertIsNotNone(openjax_tui)


if __name__ == "__main__":
    unittest.main()
