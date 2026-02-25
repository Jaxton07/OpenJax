"""Tests for package initialization."""

from __future__ import annotations

import unittest
from io import StringIO
from unittest.mock import patch


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

    def test_main_output(self) -> None:
        """Test that main function produces expected output."""
        from openjax_tui import main

        with patch("sys.stdout", new=StringIO()) as fake_stdout:
            main()
            output = fake_stdout.getvalue()

        self.assertIn("OpenJax TUI", output)
        self.assertIn("初始化成功", output)
        self.assertIn("Textual", output)


class TestModuleEntryPoint(unittest.TestCase):
    """Test module entry point functionality."""

    def test_module_execution(self) -> None:
        """Test that the module can be executed."""
        import runpy

        with patch("sys.stdout", new=StringIO()) as fake_stdout:
            try:
                runpy.run_module("openjax_tui", run_name="__main__")
            except SystemExit:
                pass  # Expected if main() calls sys.exit
            output = fake_stdout.getvalue()

        self.assertIn("OpenJax TUI", output)


if __name__ == "__main__":
    unittest.main()
