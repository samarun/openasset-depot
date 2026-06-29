from __future__ import annotations

from typing import Any, Callable

import hou

try:
    from PySide6 import QtCore, QtWidgets
except ImportError:
    from PySide2 import QtCore, QtWidgets

from . import runtime
from .bridge_loader import load_bridge


_, BridgeError, _ = load_bridge()


class OpenAssetPanel(QtWidgets.QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self._busy = False
        self._build_ui()
        self.refresh()

    def _build_ui(self) -> None:
        layout = QtWidgets.QVBoxLayout(self)
        layout.setContentsMargins(12, 12, 12, 12)
        layout.setSpacing(8)
        title = QtWidgets.QLabel("OPENASSET DEPOT")
        title.setStyleSheet("font-weight: 700; color: #70b8ad;")
        layout.addWidget(title)
        self.status = QtWidgets.QLabel("Save the scene to begin")
        self.status.setWordWrap(True)
        self.status.setMinimumHeight(42)
        layout.addWidget(self.status)

        row = QtWidgets.QHBoxLayout()
        self.refresh_button = self._button("Refresh", self.refresh)
        self.checkout_button = self._button("Check Out HIP", self.checkout_scene)
        row.addWidget(self.refresh_button)
        row.addWidget(self.checkout_button)
        layout.addLayout(row)

        self.hda_button = self._button("Check Out Selected HDA", self.checkout_hda)
        layout.addWidget(self.hda_button)
        self.sync_button = self._button("Sync Latest", self.sync)
        layout.addWidget(self.sync_button)
        self.validate_button = self._button("Validate Scene", self.validate)
        layout.addWidget(self.validate_button)

        self.description = QtWidgets.QLineEdit("Houdini scene update")
        self.description.setPlaceholderText("Submit description")
        layout.addWidget(self.description)
        self.submit_button = self._button("Submit Changes", self.submit)
        layout.addWidget(self.submit_button)
        self.revert_button = self._button("Revert HIP Checkout", self.revert)
        layout.addWidget(self.revert_button)
        layout.addStretch(1)
        self._action_widgets = [
            self.refresh_button,
            self.checkout_button,
            self.hda_button,
            self.sync_button,
            self.validate_button,
            self.submit_button,
            self.revert_button,
            self.description,
        ]

    def _button(self, label: str, callback: Callable[[], None]):
        button = QtWidgets.QPushButton(label)
        button.clicked.connect(callback)
        return button

    def refresh(self) -> None:
        self._with_scene(
            lambda bridge, path: bridge.status([path]),
            lambda values: self._status_message(values[0]),
        )

    def checkout_scene(self) -> None:
        self._with_scene(
            lambda bridge, path: bridge.checkout(path, "Editing in Houdini"),
            lambda result: f"Checked out {result['path']}",
        )

    def checkout_hda(self) -> None:
        try:
            path = runtime.selected_hda_path()
            bridge = runtime.client(path)
            self._run(
                lambda: bridge.checkout(path, "Editing HDA in Houdini"),
                lambda result: f"Checked out {result['path']}",
            )
        except BridgeError as error:
            self._set_error(str(error))

    def sync(self) -> None:
        self._with_scene(
            lambda bridge, _path: bridge.sync(timeout_seconds=1800),
            lambda result: f"Synced {result.get('synced_count', 0)} file(s)",
        )

    def validate(self) -> None:
        self._with_scene(
            lambda bridge, path: bridge.validate([path], "Houdini"),
            self._validation_message,
        )

    def submit(self) -> None:
        description = self.description.text().strip()
        if not description:
            self._set_error("Enter a submit description.")
            return
        self._with_scene(
            lambda bridge, _path: bridge.submit(description, timeout_seconds=1800),
            lambda result: f"Submitted {len(result.get('revisions', []))} file(s)",
        )

    def revert(self) -> None:
        answer = QtWidgets.QMessageBox.question(
            self,
            "Revert Checkout",
            "Remove the HIP file from the pending changelist and release its lock?",
        )
        if answer != QtWidgets.QMessageBox.Yes:
            return
        self._with_scene(
            lambda bridge, path: bridge.revert(path),
            lambda result: f"Reverted checkout for {result['path']}",
        )

    def _with_scene(self, operation, success) -> None:
        try:
            path = runtime.scene_path()
            bridge = runtime.client(path)
            self._run(lambda: operation(bridge, path), success)
        except BridgeError as error:
            self._set_error(str(error))

    def _run(self, operation, success) -> None:
        if self._busy:
            self._set_error("Another OpenAsset operation is already running.")
            return
        self._set_busy(True)
        self.status.setText("Working...")

        def completed(result: Any) -> None:
            if not self.isVisible():
                return
            self._set_busy(False)
            self.status.setStyleSheet("")
            self.status.setText(success(result))

        def failed(error: Exception) -> None:
            if not self.isVisible():
                return
            self._set_busy(False)
            self._set_error(str(error))

        runtime.run_operation(operation, on_success=completed, on_error=failed)

    def _set_busy(self, busy: bool) -> None:
        self._busy = busy
        for widget in self._action_widgets:
            widget.setEnabled(not busy)

    def _set_error(self, message: str) -> None:
        self.status.setStyleSheet("color: #d86f64;")
        self.status.setText(message)

    @staticmethod
    def _status_message(status) -> str:
        values = [status.local_state.replace("_", " ").title()]
        if status.needs_sync:
            values.append("Needs Sync")
        if status.lock_state == "mine":
            values.append("Checked Out by Me")
        elif status.lock_state == "other":
            values.append("Checked Out Elsewhere")
        return " | ".join(values)

    @staticmethod
    def _validation_message(result) -> str:
        errors = len(result.get("adapter", {}).get("errors", [])) + len(
            result.get("core", {}).get("errors", [])
        )
        warnings = len(result.get("adapter", {}).get("warnings", [])) + len(
            result.get("core", {}).get("warnings", [])
        )
        return f"Validation: {errors} error(s), {warnings} warning(s)"


def create_interface():
    return OpenAssetPanel()
