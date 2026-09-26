from __future__ import annotations

import nuke
import nukescripts

from . import commands
from .bridge_loader import load_words


_words = load_words()
ACTIONS = _words.ACTIONS
PANEL_ID = "com.openasset.depot"
_active_panel = None


class OpenAssetPanel(nukescripts.PythonPanel):
    def __init__(self):
        super().__init__(_words.PRODUCT_NAME, PANEL_ID)
        global _active_panel
        _active_panel = self
        self.status = nuke.Text_Knob("status", "Status", _words.MESSAGES["save_first"])
        self.progress = nuke.Progress_Knob("progress", "Progress")
        self.progress.setValue(0)
        self.refresh = nuke.PyScript_Knob("refresh", ACTIONS["refresh"])
        self.checkout = nuke.PyScript_Knob("checkout", _words.qualified("checkout", "Script"))
        self.sync = nuke.PyScript_Knob("sync", ACTIONS["sync"])
        self.validate = nuke.PyScript_Knob("validate", ACTIONS["validate"])
        self.description = nuke.String_Knob("description", _words.FIELDS["description"])
        self.description.setValue("Nuke script update")
        self.submit = nuke.PyScript_Knob("submit", ACTIONS["submit"])
        self.shelve = nuke.PyScript_Knob("shelve", ACTIONS["shelve"])
        self.unshelve = nuke.PyScript_Knob("unshelve", ACTIONS["unshelve"])
        self.revert = nuke.PyScript_Knob("revert", ACTIONS["revert"])
        for knob in (
            self.status,
            self.progress,
            self.refresh,
            self.checkout,
            self.sync,
            self.validate,
            self.description,
            self.submit,
            self.shelve,
            self.unshelve,
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
        elif knob is self.shelve:
            commands.shelve()
        elif knob is self.unshelve:
            commands.unshelve()
        elif knob is self.revert:
            commands.revert()

    def set_status(self, message: str) -> None:
        self.status.setValue(message)

    def set_busy(self, busy: bool) -> None:
        for knob in (self.refresh, self.checkout, self.sync, self.validate, self.submit, self.shelve, self.unshelve, self.revert):
            knob.setEnabled(not busy)

    def set_progress(self, value: int) -> None:
        self.progress.setValue(max(0, min(100, int(value))))


def create_panel():
    return OpenAssetPanel().addToPane()


def show_panel():
    return nukescripts.panels.restorePanel(PANEL_ID)


def active_panel():
    return _active_panel
