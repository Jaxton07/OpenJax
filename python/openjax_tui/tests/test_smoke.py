import os
import subprocess
import tempfile
import unittest
from pathlib import Path


class OpenJaxTuiSmokeTest(unittest.TestCase):
    def test_run_and_exit(self) -> None:
        root = Path(__file__).resolve().parents[3]
        env = os.environ.copy()
        env["PYTHONPATH"] = "python/openjax_sdk/src:python/openjax_tui/src"

        daemon_bin = root / "target" / "debug" / "openjaxd"
        if daemon_bin.exists():
            env["OPENJAX_DAEMON_CMD"] = str(daemon_bin)

        proc = subprocess.run(
            ["python3", "-m", "openjax_tui"],
            input="/exit\n",
            text=True,
            cwd=str(root),
            env=env,
            capture_output=True,
            timeout=20,
        )

        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        self.assertIn(">_ OpenJax (v", proc.stdout)
        self.assertIn("directory:", proc.stdout)
        self.assertNotIn("commands:", proc.stdout)
        self.assertNotIn("[status]", proc.stdout)
        self.assertIn("openjax_tui exited", proc.stdout)

    def test_run_and_exit_command_in_temp_workspace(self) -> None:
        root = Path(__file__).resolve().parents[3]
        env = os.environ.copy()
        sdk_src = root / "python" / "openjax_sdk" / "src"
        tui_src = root / "python" / "openjax_tui" / "src"
        env["PYTHONPATH"] = f"{sdk_src}:{tui_src}"
        env["OPENJAX_TUI_INPUT_BACKEND"] = "basic"
        env["OPENJAX_TUI_LOG_MAX_BYTES"] = "65536"

        daemon_bin = root / "target" / "debug" / "openjaxd"
        if daemon_bin.exists():
            env["OPENJAX_DAEMON_CMD"] = str(daemon_bin)

        with tempfile.TemporaryDirectory(prefix="openjax-tui-smoke-") as temp_cwd:
            proc = subprocess.run(
                ["python3", "-m", "openjax_tui"],
                input="/exit\n",
                text=True,
                cwd=temp_cwd,
                env=env,
                capture_output=True,
                timeout=20,
            )

        combined = f"{proc.stdout}\n{proc.stderr}"
        self.assertEqual(proc.returncode, 0, msg=combined)
        self.assertIn("openjax_tui exited", proc.stdout)
        self.assertNotIn("Traceback (most recent call last)", combined)
        self.assertNotIn("panicked at", combined)

if __name__ == "__main__":
    unittest.main()
