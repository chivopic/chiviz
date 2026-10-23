import json
from pathlib import Path
import subprocess
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[1]
LAUNCHER = REPO / "scripts" / "chiviz-launch"


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.bin_dir = Path(self.temp_dir.name) / "bin"
        self.bin_dir.mkdir()
        self.log_file = Path(self.temp_dir.name) / "calls.jsonl"
        self.env = {"PATH": str(self.bin_dir), "CALL_LOG": str(self.log_file)}

    def tearDown(self):
        self.temp_dir.cleanup()

    def add_command(self, name):
        command = self.bin_dir / name
        command.write_text(
            "#!/usr/bin/python3\n"
            "import json, os, pathlib, sys\n"
            "with open(os.environ['CALL_LOG'], 'a', encoding='utf-8') as log:\n"
            "    log.write(json.dumps({'command': pathlib.Path(sys.argv[0]).name, 'args': sys.argv[1:]}) + '\\n')\n",
            encoding="utf-8",
        )
        command.chmod(0o755)

    def calls(self):
        if not self.log_file.exists():
            return []
        return [json.loads(line) for line in self.log_file.read_text().splitlines()]

    def run_launcher(self, mode):
        return subprocess.run([str(LAUNCHER), mode], env=self.env, capture_output=True, text=True)

    def test_missing_chiviz_notifies_actionable_build_steps(self):
        for command in ("kitty", "omarchy-launch-or-focus", "omarchy-notification-send"):
            self.add_command(command)

        result = self.run_launcher("visualizer")

        self.assertEqual(result.returncode, 127)
        self.assertEqual([call["command"] for call in self.calls()], ["omarchy-notification-send"])
        notice = self.calls()[-1]
        self.assertEqual(notice["command"], "omarchy-notification-send")
        self.assertIn("cargo build --release", notice["args"][-1])
        self.assertIn("install -m 755 target/release/chiviz", notice["args"][-1])
        self.assertIn("~/.local/bin", notice["args"][-1])

    def test_missing_kitty_notifies_install_command(self):
        for command in ("chiviz", "omarchy-launch-or-focus", "omarchy-notification-send"):
            self.add_command(command)

        result = self.run_launcher("doctor")

        self.assertEqual(result.returncode, 127)
        self.assertEqual([call["command"] for call in self.calls()], ["omarchy-notification-send"])
        notice = self.calls()[-1]
        self.assertEqual(notice["command"], "omarchy-notification-send")
        self.assertIn("omarchy pkg add kitty", notice["args"][-1])

    def test_missing_both_dependencies_sends_combined_notice(self):
        self.add_command("omarchy-notification-send")

        result = self.run_launcher("float")

        self.assertEqual(result.returncode, 127)
        self.assertEqual([call["command"] for call in self.calls()], ["omarchy-notification-send"])
        detail = self.calls()[0]["args"][-1]
        self.assertIn("Missing chiviz and Kitty", detail)
        self.assertIn("cargo build --release", detail)
        self.assertIn("omarchy pkg add kitty", detail)

    def test_missing_dependencies_without_notifier_use_stderr(self):
        result = self.run_launcher("visualizer")

        self.assertEqual(result.returncode, 127)
        self.assertIn("Missing chiviz and Kitty", result.stderr)
        self.assertIn("omarchy pkg add kitty", result.stderr)
        self.assertEqual(self.calls(), [])

    def test_invalid_mode_does_not_launch_or_notify(self):
        self.add_command("omarchy-notification-send")
        self.add_command("kitty")
        self.add_command("chiviz")
        self.add_command("omarchy-launch-or-focus")

        result = self.run_launcher("invalid")

        self.assertEqual(result.returncode, 2)
        self.assertIn("Usage:", result.stderr)
        self.assertEqual(self.calls(), [])

    def test_all_three_actions_use_expected_argument_vectors(self):
        for command in ("chiviz", "kitty", "omarchy-launch-or-focus"):
            self.add_command(command)

        for mode in ("visualizer", "doctor", "float"):
            with self.subTest(mode=mode):
                result = self.run_launcher(mode)
                self.assertEqual(result.returncode, 0, result.stderr)

        self.assertEqual(self.calls(), [
            {"command": "omarchy-launch-or-focus", "args": ["chiviz-visualizer", "kitty --class chiviz-visualizer -e chiviz"]},
            {"command": "kitty", "args": ["--class", "chiviz-doctor", "--hold", "-e", "chiviz", "--doctor"]},
            {"command": "kitty", "args": ["--class", "chiviz-float", "-e", "chiviz"]},
        ])


if __name__ == "__main__":
    unittest.main()
