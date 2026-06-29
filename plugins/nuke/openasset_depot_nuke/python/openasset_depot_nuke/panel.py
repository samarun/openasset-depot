from __future__ import annotations

import nuke
import nukescripts

from . import commands


PANEL_ID = "com.openasset.depot"
_active_panel = None


class OpenAssetPanel(nukescripts.PythonPanel):
    def __init__(self):
        super().__init__("OpenAsset Depot", PANEL_ID)
        global _active_panel
        _active_panel = self
        self.status = nuke.Text_Knob("status", "Status", "Save the script to begin")
        self.refresh = nuke.PyScript_Knob("refresh", "Refresh Status")
        self.checkout = nuke.PyScript_Knob("checkout", "Check Out Script")
        self.sync = nuke.PyScript_Knob("sync", "Sync Latest")
        self.validate = nuke.PyScript_Knob("validate", "Validate Script")
        self.description = nuke.String_Knob("description", "Description")
        self.description.setValue("Nuke script update")
        self.submit = nuke.PyScript_Knob("submit", "Submit Changes")
        self.revert = nuke.PyScript_Knob("revert", "Revert Checkout")
        for knob in (
            self.status,
            self.refresh,
            self.checkout,
            self.sync,
            self.validate,
            self.description,
            self.submit,
            self.revert,
        ):
            self.addKnob(knob)

    def knobChanged(self, knob):
        if knob is self.refresh:
            commands.refresh()
        elif knob is self.checkout:
            commands.checkout()
        elif knob is self.sync:
            commands.sync()
        elif knob is self.validate:
            commands.validate()
        elif knob is self.submit:
            commands.submit(self.description.value())
        elif knob is self.revert:
            commands.revert()

    def set_status(self, message: str) -> None:
        self.status.setValue(message)

    def set_busy(self, busy: bool) -> None:
        for knob in (self.refresh, self.checkout, self.sync, self.validate, self.submit, self.revert):
            knob.setEnabled(not busy)


def create_panel():
    return OpenAssetPanel().addToPane()


def show_panel():
    return nukescripts.panels.restorePanel(PANEL_ID)


def active_panel():
    return _active_panel
