import nuke
import nukescripts

from openasset_depot_nuke import commands, panel
from openasset_depot_nuke.bridge_loader import load_words


words = load_words()
ACTIONS = words.ACTIONS

menu = nuke.menu("Nuke").addMenu("OpenAsset")
menu.addCommand("Workspace", panel.show_panel)
menu.addSeparator()
menu.addCommand(ACTIONS["refresh"], commands.refresh)
menu.addCommand(words.qualified("checkout", "Script"), commands.checkout)
menu.addCommand(ACTIONS["sync"], commands.sync)
menu.addCommand(ACTIONS["validate"], commands.validate)
menu.addCommand(ACTIONS["submit"], commands.submit)
menu.addCommand(ACTIONS["shelve"], commands.shelve)
menu.addCommand(ACTIONS["unshelve"], commands.unshelve)
menu.addCommand(ACTIONS["revert"], commands.revert)

nukescripts.registerPanel(panel.PANEL_ID, panel.create_panel)
nuke.addOnScriptLoad(commands.refresh_if_panel_open)
