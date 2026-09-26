#!/usr/bin/env python3
"""Blender-host smoke test for add-on registration and async UI completion."""

from __future__ import annotations

import sys
import tempfile
import time
from pathlib import Path

import bpy


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "plugins" / "common"))
sys.path.insert(0, str(ROOT / "plugins" / "blender"))

import openasset_depot_addon as addon  # noqa: E402
from openasset_depot_addon import runtime  # noqa: E402


def wait_for_worker() -> None:
    deadline = time.monotonic() + 5
    while runtime.task_runner().pending_count and time.monotonic() < deadline:
        time.sleep(0.01)
    if runtime.task_runner().pending_count:
        raise AssertionError("Blender integration worker did not complete")
    runtime._drain_callbacks()


def fail_operation():
    raise RuntimeError("simulated submit failure")


class FakeStatus:
    def __init__(self, local_state="untracked") -> None:
        self.local_state = local_state
        self.pending_action = None
        self.needs_sync = False
        self.lock_state = None


class FakeBridge:
    def __init__(self, local_state="untracked") -> None:
        self.local_state = local_state
        self.added = []
        self.checked_out = []
        self.submitted = []
        self.previews = []

    def status(self, paths):
        return [FakeStatus(self.local_state) for _path in paths]

    def pending(self):
        return {"files": [{"path": path, "action": "add"} for path in self.added]}

    def add(self, path):
        self.added.append(path)
        return {"path": path}

    def checkout(self, path, reason=None):
        self.checked_out.append((path, reason))
        self.added.append(path)
        return {"path": path}

    def submit(self, description, timeout_seconds=None):
        self.submitted.append((description, timeout_seconds))
        return {"revisions": [{"path": path} for path in self.added]}

    def upload_preview(self, path, image):
        self.previews.append((path, Path(image).is_file()))
        return {"path": path}


class LegacyPreviewBridge(FakeBridge):
    upload_preview = None


addon.register()
try:
    state = bpy.context.window_manager.openasset_depot

    runtime.run_operation(
        lambda: {"revisions": ["scene.blend"]},
        success=lambda result: f"Submitted {len(result['revisions'])} file(s)",
        label="Submitting changes",
    )
    wait_for_worker()
    assert not state.busy
    assert state.message == "Submitted 1 file(s)"
    assert state.progress == 100.0
    assert state.status_kind == "success"

    runtime.run_operation(
        fail_operation,
        success=lambda _result: "unexpected success",
        label="Submitting changes",
    )
    wait_for_worker()
    assert not state.busy
    assert state.message == "simulated submit failure"
    assert state.progress == 0.0
    assert state.status_kind == "error"

    runtime.run_operation(
        lambda: (_ for _ in ()).throw(
            runtime.BridgeError(
                "Your OpenAsset session expired. Open the OpenAsset Depot desktop app, sign out, and sign in again."
            )
        ),
        success=lambda _result: "unexpected success",
        label="Refreshing status",
    )
    wait_for_worker()
    assert state.needs_reauth
    assert "session expired" in state.message

    assert hasattr(bpy.ops.openasset, "add")
    assert hasattr(bpy.ops.openasset, "open_desktop")

    with tempfile.TemporaryDirectory() as directory:
        scene = str(Path(directory) / "NewScene.blend")
        bpy.ops.wm.save_as_mainfile(filepath=scene)
        fake = FakeBridge()
        original_client = runtime.client
        runtime.client = lambda: fake
        try:
            state.submit_description = "Initial Blender scene"
            assert bpy.ops.openasset.submit() == {"FINISHED"}
            wait_for_worker()
        finally:
            runtime.client = original_client
        assert fake.added == [scene]
        assert fake.submitted == [("Initial Blender scene", 1800)]
        assert fake.previews == [(scene, True)]
        assert state.message == "Submitted 1 file(s) · Preview ready"

        modified = FakeBridge(local_state="modified")
        runtime.client = lambda: modified
        original_preview = runtime.generate_scene_preview
        original_review_proxy = runtime.generate_scene_review_proxy
        runtime.generate_scene_preview = lambda: (None, None)
        runtime.generate_scene_review_proxy = lambda: (None, None)
        try:
            assert bpy.ops.openasset.submit() == {"FINISHED"}
            wait_for_worker()
        finally:
            runtime.client = original_client
            runtime.generate_scene_preview = original_preview
            runtime.generate_scene_review_proxy = original_review_proxy
        assert modified.checked_out == [(scene, "Automatic checkout from Blender submit")]
        assert modified.submitted == [("Initial Blender scene", 1800)]
        assert state.message == "Submitted 1 file(s)"

        legacy = LegacyPreviewBridge(local_state="modified")
        runtime.client = lambda: legacy
        runtime.generate_scene_preview = lambda: (Path(directory) / "missing-preview.png", None)
        runtime.generate_scene_review_proxy = lambda: (None, None)
        try:
            assert bpy.ops.openasset.submit() == {"FINISHED"}
            wait_for_worker()
        finally:
            runtime.client = original_client
            runtime.generate_scene_preview = original_preview
            runtime.generate_scene_review_proxy = original_review_proxy
        assert state.message == "Submitted 1 file(s) · Preview unavailable"

    print("OpenAsset Blender add-on smoke test passed")
finally:
    addon.unregister()
