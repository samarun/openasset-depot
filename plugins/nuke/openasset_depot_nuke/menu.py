import nuke
import nukescripts

from openasset_depot_nuke import commands, panel


menu = nuke.menu("Nuke").addMenu("OpenAsset")
menu.addCommand("Workspace", panel.show_panel)
menu.addSeparator()
menu.addCommand("Refresh Script Status", commands.refresh)
menu.addCommand("Check Out Script", commands.checkout)
menu.addCommand("Sync Latest", commands.sync)
menu.addCommand("Validate Script", commands.validate)
menu.addCommand("Submit Changes", commands.submit)
menu.addCommand("Revert Checkout", commands.revert)

nukescripts.registerPanel(panel.PANEL_ID, panel.create_panel)
nuke.addOnScriptLoad(commands.refresh_if_panel_open)
