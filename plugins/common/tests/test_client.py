import json
import stat
import tempfile
import threading
import unittest
from pathlib import Path

from openasset_depot_bridge import (
    BridgeClient,
    BridgeError,
    BridgeProtocolError,
    CallbackQueue,
    TaskRunner,
    find_workspace,
)


FAKE_CLI = """#!/usr/bin/env python3
import json
import sys

args = sys.argv[1:]
integration = args.index('integration')
operation = args[integration + 3] if args[integration + 1] == '--protocol-version' else args[integration + 1]
if operation == 'fail':
    print(json.dumps({'protocol_version': 2, 'type': 'result', 'ok': False, 'data': None, 'error': 'locked elsewhere'}))
    raise SystemExit(1)
if operation == 'status':
    data = {'files': [{'path': args[-1], 'local_state': 'clean', 'pending_action': None,
                       'local_revision': 2, 'remote_revision': 2, 'needs_sync': False,
                       'remote_deleted': False,
                       'lock_state': 'mine', 'lock_reason': 'Lighting'}]}
else:
    data = {'workspace_id': 'workspace-1', 'depot': 'show', 'stream': 'main',
            'root': '.', 'active_changelist_id': None, 'tracked_file_count': 1,
            'pending_file_count': 0, 'server_url': 'http://127.0.0.1:8080',
            'username': 'artist', 'authenticated': True, 'args': args}
print(json.dumps({'protocol_version': 2, 'type': 'progress', 'operation': operation, 'phase': 'starting',
                  'message': 'Starting ' + operation, 'completed': 5, 'total': 100}), flush=True)
print(json.dumps({'protocol_version': 2, 'type': 'result', 'ok': True, 'data': data, 'error': None}))
"""


class BridgeClientTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        (self.root / ".oad").mkdir()
        (self.root / ".oad" / "workspace.json").write_text("{}", encoding="utf-8")
        self.cli = self.root / "oad-fake"
        self.cli.write_text(FAKE_CLI, encoding="utf-8")
        self.cli.chmod(self.cli.stat().st_mode | stat.S_IXUSR)

    def tearDown(self):
        self.temp.cleanup()

    def test_finds_workspace_from_nested_file(self):
        nested = self.root / "Scenes" / "shot.blend"
        nested.parent.mkdir()
        nested.write_bytes(b"scene")
        self.assertEqual(find_workspace(nested), self.root)

    def test_parses_context_and_status(self):
        progress = []
        client = BridgeClient(self.root, cli_path=self.cli, progress_callback=progress.append)
        self.assertEqual(client.context().workspace_id, "workspace-1")
        status = client.status(["Scenes/shot.blend"])
        self.assertEqual(status[0].local_revision, 2)
        self.assertEqual(status[0].lock_state, "mine")
        self.assertEqual(progress[-1].operation, "status")
        self.assertEqual(progress[-1].completed, 5)

    def test_refuses_unbounded_status_scan(self):
        client = BridgeClient(self.root, cli_path=self.cli)
        with self.assertRaises(BridgeError):
            client.status([])

    def test_uploads_portable_review_proxy_through_cli_protocol(self):
        media = self.root / "shot.glb"
        media.write_bytes(b"glTF")
        result = BridgeClient(self.root, cli_path=self.cli).upload_review_proxy(
            self.root / "shot.blend",
            media,
        )
        self.assertIn("review-proxy", result["args"])
        self.assertIn("--media", result["args"])

    def test_rejects_unsupported_protocol(self):
        self.cli.write_text(
            "#!/usr/bin/env python3\nimport json\nprint(json.dumps({'protocol_version': 99, 'type': 'result', 'ok': True, 'data': {}}))\n",
            encoding="utf-8",
        )
        self.cli.chmod(self.cli.stat().st_mode | stat.S_IXUSR)
        with self.assertRaises(BridgeProtocolError):
            BridgeClient(self.root, cli_path=self.cli).context()

    def test_translates_expired_session_into_artist_recovery_copy(self):
        self.cli.write_text(
            "#!/usr/bin/env python3\n"
            "import json\n"
            "print(json.dumps({'protocol_version': 2, 'type': 'result', 'ok': False, "
            "'data': None, 'error': 'request failed with 401 Unauthorized: ExpiredSignature'}))\n"
            "raise SystemExit(1)\n",
            encoding="utf-8",
        )
        self.cli.chmod(self.cli.stat().st_mode | stat.S_IXUSR)
        with self.assertRaisesRegex(BridgeError, "session expired.*desktop app"):
            BridgeClient(self.root, cli_path=self.cli).context()

    def test_task_runner_schedules_success_on_host_callback(self):
        runner = TaskRunner("test-openasset")
        calls = []
        completed = runner.submit(
            lambda: "ready",
            schedule=lambda callback: callback(),
            on_success=calls.append,
        )
        completed.result(timeout=2)
        self.assertEqual(calls, ["ready"])
        runner.shutdown()

    def test_callback_queue_delivers_worker_result_on_draining_thread(self):
        runner = TaskRunner("test-openasset-queue")
        callbacks = CallbackQueue()
        calls = []
        scheduled = threading.Event()

        def schedule(callback):
            callbacks.schedule(callback)
            scheduled.set()

        completed = runner.submit(
            lambda: "ready",
            schedule=schedule,
            on_success=calls.append,
        )
        completed.result(timeout=2)
        self.assertTrue(scheduled.wait(timeout=2))
        self.assertEqual(calls, [])
        self.assertFalse(callbacks.empty)
        self.assertEqual(callbacks.drain(), 1)
        self.assertEqual(calls, ["ready"])
        self.assertTrue(callbacks.empty)
        runner.shutdown()


if __name__ == "__main__":
    unittest.main()
