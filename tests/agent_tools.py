"""Agent kit regressions; all destinations and transport fixtures are temporary."""
import base64
import contextlib
import importlib.util
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m
installer = load('installer', ROOT / 'scripts/install-agent-skills.py')
smoke = load('smoke', ROOT / 'skills/e2evm-setup/scripts/verify-browser.py')

class InstallerTests(unittest.TestCase):

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def run_install(self, *args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return installer.main(list(args))

    def test_inventory_without_target_writes_nothing(self):
        self.assertEqual(self.run_install('--list'), 0)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_target_required(self):
        with self.assertRaises(SystemExit) as e:
            self.run_install()
        self.assertEqual(e.exception.code, 2)

    def test_dry_run_creates_no_parent(self):
        d = self.root / 'new parent' / 'skills'
        self.assertEqual(self.run_install('--dest', str(d), '--dry-run'), 0)
        self.assertFalse(d.parent.exists())

    def test_selected_skill_complete_at_path_with_spaces(self):
        d = self.root / 'project with spaces' / 'skills'
        self.assertEqual(self.run_install('--dest', str(d), '--skills', 'guest-e2e'), 0)
        self.assertTrue((d / 'guest-e2e/references/browser-smoke.md').is_file())
        self.assertFalse((d / 'e2evm-setup').exists())

    def test_both_skills_complete(self):
        d = self.root / 'skills'
        self.assertEqual(self.run_install('--dest', str(d)), 0)
        self.assertEqual((d / 'e2evm-setup/scripts/verify-browser.py').read_bytes(), (ROOT / 'skills/e2evm-setup/scripts/verify-browser.py').read_bytes())
        self.assertTrue((d / 'guest-e2e/agents/openai.yaml').is_file())

    def test_collision_preflights_every_selection(self):
        d = self.root / 'skills'
        (d / 'guest-e2e').mkdir(parents=True)
        sentinel = d / 'guest-e2e/keep'
        sentinel.write_text('user edits')
        self.assertEqual(self.run_install('--dest', str(d)), 1)
        self.assertEqual(sentinel.read_text(), 'user edits')
        self.assertFalse((d / 'e2evm-setup').exists())

    def test_dangling_symlink_is_collision(self):
        d = self.root / 'skills'
        d.mkdir()
        (d / 'e2evm-setup').symlink_to(self.root / 'absent')
        self.assertEqual(self.run_install('--dest', str(d)), 1)
        self.assertTrue((d / 'e2evm-setup').is_symlink())
        self.assertFalse((d / 'guest-e2e').exists())

    def test_file_collision(self):
        d = self.root / 'file'
        d.write_text('keep')
        self.assertEqual(self.run_install('--dest', str(d)), 1)
        self.assertEqual(d.read_text(), 'keep')

    def test_repeated_install_does_not_overwrite(self):
        d = self.root / 'skills'
        self.run_install('--dest', str(d))
        f = d / 'guest-e2e/SKILL.md'
        f.write_text('custom')
        self.assertEqual(self.run_install('--dest', str(d)), 1)
        self.assertEqual(f.read_text(), 'custom')

    def test_invalid_and_duplicate_selections(self):
        for selection in (['../outside'], ['unknown'], ['guest-e2e', 'guest-e2e']):
            with self.assertRaises(SystemExit) as e:
                self.run_install('--dest', str(self.root / 'skills'), '--skills', *selection)
            self.assertEqual(e.exception.code, 2)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_relative_destination(self):
        old = Path.cwd()
        try:
            os.chdir(self.root)
            self.assertEqual(self.run_install('--dest', 'project/skills', '--skills', 'e2evm-setup'), 0)
        finally:
            os.chdir(old)
        self.assertTrue((self.root / 'project/skills/e2evm-setup/SKILL.md').is_file())

    def test_cli_works_outside_repository(self):
        result = subprocess.run(
            [sys.executable, str(ROOT / 'scripts/install-agent-skills.py'),
             '--dest', 'project/skills', '--skills', 'e2evm-setup'],
            cwd=self.root, capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.root / 'project/skills/e2evm-setup/SKILL.md').is_file())

    def test_personal_targets_in_temporary_home(self):
        with patch.object(installer.Path, 'home', return_value=self.root), patch.dict(os.environ, {'CODEX_HOME': str(self.root / 'custom-codex')}):
            self.assertEqual(self.run_install('--agent', 'codex', '--skills', 'e2evm-setup'), 0)
            self.assertEqual(self.run_install('--agent', 'claude', '--skills', 'guest-e2e'), 0)
        self.assertTrue((self.root / 'custom-codex/skills/e2evm-setup/SKILL.md').is_file())
        self.assertTrue((self.root / '.claude/skills/guest-e2e/SKILL.md').is_file())

    def test_nested_source_destination_refused(self):
        self.assertEqual(self.run_install('--dest', str(ROOT / 'skills/e2evm-setup/nested'), '--dry-run'), 1)

    def test_symlink_source_refused(self):
        fake = self.root / 'source'
        s = fake / 'e2evm-setup'
        s.mkdir(parents=True)
        (s / 'SKILL.md').write_text('skill')
        (s / 'link').symlink_to('/tmp')
        with patch.object(installer, 'SKILLS_ROOT', fake):
            self.assertEqual(self.run_install('--dest', str(self.root / 'dest'), '--skills', 'e2evm-setup'), 1)
        self.assertFalse((self.root / 'dest').exists())

class SmokeTests(unittest.TestCase):

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def invoke(self, payload=None, reply=None, returncode=0, error=None):
        # Transport fixtures test artifact handling, not JPEG rendering. Live
        # guest verification and image inspection are separate integration checks.
        if payload is None:
            payload = {'status': 'passed', 'assertions': 2, 'screenshot_base64': base64.b64encode(b'\xff\xd8protocol-fixture').decode()}
        if reply is None:
            reply = {'ok': True, 'exit_code': 0, 'timed_out': False, 'truncated': False, 'output': json.dumps(payload)}
        result = subprocess.CompletedProcess([], returncode, json.dumps(reply), '')
        with patch.object(smoke.subprocess, 'run', return_value=result, side_effect=error) as run, contextlib.redirect_stdout(io.StringIO()):
            code = smoke.main(['--profile', 'dedicated', '--cli', '/path with spaces/e2evm', '--output-dir', str(self.root / 'artifacts')])
        return (code, json.loads((self.root / 'artifacts/checks.json').read_text()), run)

    def test_success_preserves_exact_guest_argv_and_exports(self):
        (code, data, run) = self.invoke()
        self.assertEqual(code, 0)
        self.assertEqual(data['assertions'], 2)
        self.assertTrue(Path(data['host_screenshot']).is_file())
        argv = run.call_args.args[0]
        self.assertEqual(argv[:6], ['/path with spaces/e2evm', 'exec', 'dedicated', '--', '/home/harbor/Workspace/.venv/bin/python', '-c'])
        compile(argv[6], '<guest-script>', 'exec')

    def test_assertion_failure_still_exports_image(self):
        (code, data, _) = self.invoke({'status': 'failed', 'assertions': 1, 'error': 'heading mismatch', 'screenshot_base64': base64.b64encode(b'\xff\xd8protocol-fixture').decode()})
        self.assertEqual(code, 1)
        self.assertEqual(data['status'], 'failed')
        self.assertEqual(data['error'], 'heading mismatch')
        self.assertTrue(Path(data['host_screenshot']).exists())

    def test_missing_image_blocks_even_passed_assertions(self):
        (code, data, _) = self.invoke({'status': 'passed', 'assertions': 2, 'capture_error': 'capture failed'})
        self.assertEqual(code, 1)
        self.assertEqual(data['status'], 'blocked')

    def test_truncated_transport_cannot_pass(self):
        (code, data, _) = self.invoke(reply={'ok': True, 'exit_code': 0, 'truncated': True, 'output': 'passed'})
        self.assertEqual(code, 1)
        self.assertEqual(data['status'], 'blocked')

    def test_process_timeout_is_blocked(self):
        (code, data, _) = self.invoke(error=subprocess.TimeoutExpired('e2evm', 90))
        self.assertEqual(code, 1)
        self.assertEqual(data['status'], 'blocked')

    def test_invalid_image_blocks(self):
        (code, data, _) = self.invoke({'status': 'passed', 'assertions': 2, 'screenshot_base64': 'not base64!'})
        self.assertEqual(code, 1)
        self.assertEqual(data['status'], 'blocked')

    def test_existing_artifact_directory_not_overwritten(self):
        (self.root / 'artifacts').mkdir()
        (self.root / 'artifacts/keep').write_text('keep')
        with patch.object(smoke.subprocess, 'run') as run, contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            smoke.main(['--profile', 'x', '--output-dir', str(self.root / 'artifacts')])
        run.assert_not_called()
        self.assertEqual((self.root / 'artifacts/keep').read_text(), 'keep')
if __name__ == '__main__':
    unittest.main()
