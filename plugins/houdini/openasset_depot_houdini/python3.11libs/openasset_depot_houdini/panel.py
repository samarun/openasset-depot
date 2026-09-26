from __future__ import annotations

from typing import Any, Callable

import hou

try:
    from PySide6 import QtCore, QtWidgets
except ImportError:
    from PySide2 import QtCore, QtWidgets

from . import runtime
from .bridge_loader import load_bridge, load_theme, load_words


_, BridgeError, _ = load_bridge()
_words = load_words()
_theme = load_theme()
ACTIONS = _words.ACTIONS


class OpenAssetPanel(QtWidgets.QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self._busy = False
        self._build_ui()
        self.refresh()

    def _build_ui(self) -> None:
        # The shared stylesheet only matches these object names, which is what
        # keeps it from leaking into Houdini's own widgets.
        self.setObjectName("OadPanel")
        self.setStyleSheet(_theme.qt_stylesheet())
        layout = QtWidgets.QVBoxLayout(self)
        layout.setContentsMargins(12, 12, 12, 12)
        layout.setSpacing(8)
        title = QtWidgets.QLabel(_words.PRODUCT_NAME)
        title.setObjectName("OadTitle")
        layout.addWidget(title)
        self.status = QtWidgets.QLabel("Save the scene to begin")
        self.status.setObjectName("OadSubtle")
        self.status.setWordWrap(True)
        self.status.setMinimumHeight(42)
        layout.addWidget(self.status)
        self.progress = QtWidgets.QProgressBar()
        self.progress.setRange(0, 100)
        self.progress.setTextVisible(True)
        self.progress.hide()
        layout.addWidget(self.progress)

        row = QtWidgets.QHBoxLayout()
        self.refresh_button = self._button(ACTIONS["refresh"], self.refresh)
        # Houdini is the one host with two checkout targets, so both keep the
        # canonical verb and name the thing they act on.
        self.checkout_button = self._button(_words.qualified("checkout", "HIP"), self.checkout_scene)
        self.checkout_button.setObjectName("OadPrimary")
        row.addWidget(self.refresh_button)
        row.addWidget(self.checkout_button)
        layout.addLayout(row)

        self.hda_button = self._button(_words.qualified("checkout", "Selected HDA"), self.checkout_hda)
        layout.addWidget(self.hda_button)
        self.sync_button = self._button(ACTIONS["sync"], self.sync)
        layout.addWidget(self.sync_button)
        self.validate_button = self._button(ACTIONS["validate"], self.validate)
        layout.addWidget(self.validate_button)

        self.description = QtWidgets.QLineEdit("Houdini scene update")
        self.description.setPlaceholderText(_words.FIELDS["description"])
        layout.addWidget(self.description)
        self.submit_button = self._button(ACTIONS["submit"], self.submit)
        layout.addWidget(self.submit_button)
        self.shelve_button = self._button(ACTIONS["shelve"], self.shelve)
        layout.addWidget(self.shelve_button)
        self.unshelve_button = self._button(ACTIONS["unshelve"], self.unshelve)
        layout.addWidget(self.unshelve_button)
        self.revert_button = self._button(ACTIONS["revert"], self.revert)
        layout.addWidget(self.revert_button)
        layout.addStretch(1)
        self._action_widgets = [
            self.refresh_button,
            self.checkout_button,
            self.hda_button,
            self.sync_button,
            self.validate_button,
            self.submit_button,
            self.shelve_button,
            self.unshelve_button,
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
            _words.progress_for("refresh"),
        )

    def checkout_scene(self) -> None:
        self._with_scene(
            lambda bridge, path: bridge.checkout(path, "Editing in Houdini"),
            lambda result: f"Checked out {result['path']}",
            _words.progress_for("checkout", "HIP"),
        )

    def checkout_hda(self) -> None:
        try:
            path = runtime.selected_hda_path()
            bridge = runtime.client(path, self._progress_from_worker)
            self._run(
                lambda: bridge.checkout(path, "Editing HDA in Houdini"),
                lambda result: f"Checked out {result['path']}",
                _words.progress_for("checkout", "HDA"),
            )
        except BridgeError as error:
            self._set_error(str(error))

    def sync(self) -> None:
        self._with_scene(
            lambda bridge, _path: bridge.sync(timeout_seconds=1800),
            lambda result: f"Synced {result.get('synced_count', 0)} file(s)",
            _words.progress_for("sync"),
        )

    def validate(self) -> None:
        self._with_scene(
            lambda bridge, path: bridge.validate([path], "Houdini"),
            self._validation_message,
            _words.progress_for("validate"),
        )

    def submit(self) -> None:
        description = self.description.text().strip()
        if not description:
            self._set_error(_words.MESSAGES["need_description"])
            return
        self._with_scene(
            lambda bridge, _path: bridge.submit(description, timeout_seconds=1800),
            lambda result: f"Submitted {len(result.get('revisions', []))} file(s)",
            _words.progress_for("submit"),
        )

    def shelve(self) -> None:
        self._with_scene(
            lambda bridge, _path: bridge.shelve(timeout_seconds=1800),
            lambda result: f"Shelved {len(result.get('files', []))} file(s)",
            _words.progress_for("shelve"),
        )

    def unshelve(self) -> None:
        self._with_scene(
            lambda bridge, _path: bridge.unshelve(timeout_seconds=1800),
            lambda result: f"Restored {result.get('restored_count', 0)} file(s)",
            _words.progress_for("unshelve"),
        )

    def revert(self) -> None:
        answer = QtWidgets.QMessageBox.question(
            self,
            ACTIONS["revert"],
            "Remove the HIP file from the pending changelist and release its lock?",
        )
        if answer != QtWidgets.QMessageBox.Yes:
            return
        self._with_scene(
            lambda bridge, path: bridge.revert(path),
            lambda result: f"Reverted {result['path']}",
            _words.progress_for("revert"),
        )

    def _with_scene(self, operation, success, label) -> None:
        try:
            path = runtime.scene_path()
            bridge = runtime.client(path, self._progress_from_worker)
            self._run(lambda: operation(bridge, path), success, label)
        except BridgeError as error:
            self._set_error(str(error))

    def _run(self, operation, success, label) -> None:
        if self._busy:
            self._set_error(f"Another {_words.SHORT_NAME} operation is already running.")
            return
        self._set_busy(True)
        self._set_progress_value(2, label)

        def completed(result: Any) -> None:
            if not self.isVisible():
                return
            self._set_busy(False)
            self.status.setStyleSheet("")
            self._set_progress_value(100, success(result))

        def failed(error: Exception) -> None:
            if not self.isVisible():
                return
            self._set_busy(False)
            self.progress.hide()
            self._set_error(str(error))

        runtime.run_operation(operation, on_success=completed, on_error=failed)

    def _progress_from_worker(self, progress) -> None:
        runtime.defer(lambda progress=progress: self._set_progress_value(progress.completed, progress.message))

    def _set_progress_value(self, value: int, message: str) -> None:
        if not self.isVisible():
            return
        self.progress.show()
        self.progress.setValue(max(0, min(100, int(value))))
        self.progress.setFormat(f"{int(value)}%")
        self.status.setStyleSheet("")
        self.status.setText(message)

    def _set_busy(self, busy: bool) -> None:
        self._busy = busy
        for widget in self._action_widgets:
            widget.setEnabled(not busy)

    def _set_error(self, message: str) -> None:
        self.status.setStyleSheet(f"color: {_theme.hex_color('red')};")
        self.status.setText(message)

    @staticmethod
    def _status_message(status) -> str:
        return _words.status_from_bridge(status)

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
