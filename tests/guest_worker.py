import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("worker", Path(__file__).parents[1]/"guest/worker.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)

class WorkerTests(unittest.TestCase):
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
