"""Shared visual identity for the native host panels.

Maya, Houdini, Nuke, and Blender each draw their panel with a different toolkit,
so they cannot share a stylesheet the way the Adobe and Resolve HTML panels can.
What they can share is the palette, which is what actually makes the integration
recognisable: previously Houdini used a hand-picked teal, Maya a different
hand-picked teal expressed as float RGB, and Unity a third set of dot colours.

Colours match ``plugins/common/visual-kit/panel.css`` and the desktop app's dark
theme, so an artist sees one product across every surface.
"""

from __future__ import annotations

from types import MappingProxyType
from typing import Mapping, Tuple

# Hex is the source of truth; the other representations are derived so they
# cannot fall out of step.
PALETTE: Mapping[str, str] = MappingProxyType(
    {
        "bg": "#1c1c1e",
        "surface": "#242426",
        "surface_soft": "#2c2c2e",
        "surface_strong": "#37373a",
        "text": "#f5f5f7",
        "muted": "#aeaeb2",
        "line": "#3d3d40",
        "primary": "#7bd0c4",
        "primary_strong": "#9fe3da",
        # The light theme's primary, kept here for hosts that paint their own
        # label colour over a button fill and so need the accent to be the dark
        # one. Maya is the case: `cmds.button` always draws light text.
        "primary_deep": "#087f73",
        "green": "#80c99a",
        "amber": "#e0b15e",
        "red": "#ef8a85",
    }
)

# Which palette entry each status should be drawn in. Kept next to the words
# module's status list so a new status cannot ship without a colour.
STATUS_COLORS: Mapping[str, str] = MappingProxyType(
    {
        "Up to Date": "green",
        "Needs Sync": "amber",
        "Checked Out": "primary",
        "In Use": "red",
        "Ready to Submit": "amber",
        "Blocked": "red",
        "Marked for Delete": "red",
        "New File": "muted",
    }
)


def hex_color(name: str) -> str:
    """Returns a palette colour as ``#rrggbb``."""

    return PALETTE[name]


def rgb_floats(name: str) -> Tuple[float, float, float]:
    """Returns a palette colour as 0..1 floats.

    Maya's ``cmds`` widgets take ``backgroundColor`` as a float triple, which is
    why this conversion exists rather than each host doing its own arithmetic.
    """

    value = PALETTE[name].lstrip("#")
    return tuple(int(value[index : index + 2], 16) / 255.0 for index in (0, 2, 4))  # type: ignore[return-value]


def rgb_bytes(name: str) -> Tuple[int, int, int]:
    """Returns a palette colour as 0..255 integers."""

    value = PALETTE[name].lstrip("#")
    return tuple(int(value[index : index + 2], 16) for index in (0, 2, 4))  # type: ignore[return-value]


def status_color(status: str) -> str:
    """Maps a canonical status label to its palette hex value."""

    return PALETTE[STATUS_COLORS.get(status, "muted")]


def qt_stylesheet() -> str:
    """Builds the Qt stylesheet for the PySide panels (currently Houdini).

    Scoped by object name prefix so it cannot restyle the host's own widgets:
    a stylesheet that leaked into Houdini's parameter editor would be a far worse
    bug than an unstyled panel.
    """

    return f"""
    QWidget#OadPanel {{
        background: {PALETTE["bg"]};
        color: {PALETTE["text"]};
    }}
    QWidget#OadPanel QLabel {{
        color: {PALETTE["text"]};
        font-size: 12px;
    }}
    QWidget#OadPanel QLabel#OadTitle {{
        color: {PALETTE["primary"]};
        font-size: 13px;
        font-weight: 600;
    }}
    QWidget#OadPanel QLabel#OadSubtle {{
        color: {PALETTE["muted"]};
        font-size: 11px;
    }}
    QWidget#OadPanel QPushButton {{
        background: {PALETTE["surface_soft"]};
        color: {PALETTE["text"]};
        border: 1px solid {PALETTE["line"]};
        border-radius: 6px;
        padding: 7px 12px;
        min-height: 22px;
    }}
    QWidget#OadPanel QPushButton:hover:enabled {{
        background: {PALETTE["surface_strong"]};
    }}
    QWidget#OadPanel QPushButton:disabled {{
        color: {PALETTE["muted"]};
    }}
    QWidget#OadPanel QPushButton#OadPrimary {{
        background: {PALETTE["primary"]};
        border-color: {PALETTE["primary"]};
        color: #10201e;
        font-weight: 600;
    }}
    QWidget#OadPanel QPushButton#OadPrimary:hover:enabled {{
        background: {PALETTE["primary_strong"]};
        border-color: {PALETTE["primary_strong"]};
    }}
    QWidget#OadPanel QLineEdit, QWidget#OadPanel QPlainTextEdit {{
        background: {PALETTE["bg"]};
        color: {PALETTE["text"]};
        border: 1px solid {PALETTE["line"]};
        border-radius: 6px;
        padding: 6px;
    }}
    """.strip()
