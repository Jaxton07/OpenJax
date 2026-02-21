import os
import subprocess
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
            input="/help\n/exit\n",
            text=True,
            cwd=str(root),
            env=env,
            capture_output=True,
            timeout=20,
        )

        self.assertEqual(proc.returncode, 0, msg=proc.stderr)
        self.assertIn("OpenJax TUI", proc.stdout)
        self.assertIn("commands:", proc.stdout)
        self.assertIn("openjax_tui exited", proc.stdout)


if __name__ == "__main__":
    unittest.main()
