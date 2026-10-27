import importlib.util
from pathlib import Path
import tempfile
import subprocess
import sys
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("worker", Path(__file__).parents[1]/"guest/worker.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)

class WorkerTests(unittest.TestCase):
    def test_timeout_after_command_closes_output(self):
        real_popen = subprocess.Popen
        with tempfile.TemporaryDirectory() as directory:
            def launch(*args, **kwargs):
                kwargs["cwd"] = directory
                return real_popen(*args, **kwargs)
            started = time.monotonic()
            with patch.object(worker.subprocess, "Popen", side_effect=launch):
                result = worker.execute({"version": 1, "expires_at": time.time() + 10,
                    "timeout": 1, "argv": [sys.executable, "-c",
                    "import os,time; os.close(1); os.close(2); time.sleep(3)"]})
            self.assertTrue(result["timed_out"])
            self.assertLess(time.monotonic() - started, 2.5)
    def test_stale_temporary_reply_does_not_block_recovery(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/"heartbeat.json"
            path.with_suffix(".tmp").write_text("interrupted")
            worker.write_reply(path, {"timestamp": 1})
            self.assertEqual(worker.read_regular(path), {"timestamp": 1})
    def test_expired_request_never_executes(self):
        with self.assertRaisesRegex(ValueError, "expired"):
            worker.execute({"version":1,"expires_at":0,"argv":["touch","/should-not-exist"]})
    def test_reject_symlink_request(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/"real").write_text('{}')
            (root/"link").symlink_to(root/"real")
            with self.assertRaises(OSError):
                worker.read_regular(root/"link")
    def test_bounds_request(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/"large"
            path.write_text('x'*65537)
            with self.assertRaisesRegex(ValueError, "large"):
                worker.read_regular(path)

if __name__ == '__main__': unittest.main()
