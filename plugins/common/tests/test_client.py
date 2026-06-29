import json
import stat
import tempfile
import unittest
from pathlib import Path

from openasset_depot_bridge import (
    BridgeClient,
    BridgeError,
    BridgeProtocolError,
    TaskRunner,
    find_workspace,
)


FAKE_CLI = """#!/usr/bin/env python3
import json
import sys

args = sys.argv[1:]
operation = args[args.index('integration') + 1]
if operation == 'fail':
    print(json.dumps({'protocol_version': 1, 'ok': False, 'data': None, 'error': 'locked elsewhere'}))
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
            'username': 'artist', 'authenticated': True}
print(json.dumps({'protocol_version': 1, 'ok': True, 'data': data, 'error': None}))
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
        client = BridgeClient(self.root, cli_path=self.cli)
        self.assertEqual(client.context().workspace_id, "workspace-1")
        status = client.status(["Scenes/shot.blend"])
        self.assertEqual(status[0].local_revision, 2)
        self.assertEqual(status[0].lock_state, "mine")

    def test_refuses_unbounded_status_scan(self):
        client = BridgeClient(self.root, cli_path=self.cli)
        with self.assertRaises(BridgeError):
            client.status([])

    def test_rejects_unsupported_protocol(self):
        self.cli.write_text(
            "#!/usr/bin/env python3\nimport json\nprint(json.dumps({'protocol_version': 99, 'ok': True, 'data': {}}))\n",
            encoding="utf-8",
        )
        self.cli.chmod(self.cli.stat().st_mode | stat.S_IXUSR)
        with self.assertRaises(BridgeProtocolError):
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


if __name__ == "__main__":
    unittest.main()
