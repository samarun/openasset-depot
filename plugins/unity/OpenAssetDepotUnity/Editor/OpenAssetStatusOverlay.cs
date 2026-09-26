using System.Collections.Generic;
using UnityEditor;
using UnityEngine;

namespace OpenAssetDepot.Unity
{
    [InitializeOnLoad]
    internal static class OpenAssetStatusCache
    {
        private static readonly Dictionary<string, FileStatus> Statuses = new Dictionary<string, FileStatus>();

        static OpenAssetStatusCache()
        {
            EditorApplication.projectWindowItemOnGUI += Draw;
        }

        internal static void Set(string path, FileStatus status)
        {
            Statuses[path] = status;
            EditorApplication.RepaintProjectWindow();
        }

        // Mirrors the palette in plugins/common/openasset_depot_bridge/theme.py,
        // so a project-window dot is the colour the panels use for that status.
        private static readonly Color InUse = Hex(0xef, 0x8a, 0x85);
        private static readonly Color CheckedOut = Hex(0x7b, 0xd0, 0xc4);
        private static readonly Color NeedsSync = Hex(0xe0, 0xb1, 0x5e);
        private static readonly Color UpToDate = Hex(0x80, 0xc9, 0x9a);
        private static readonly Color Untracked = Hex(0xae, 0xae, 0xb2);

        private static Color Hex(byte red, byte green, byte blue)
        {
            return new Color(red / 255f, green / 255f, blue / 255f);
        }

        private static void Draw(string guid, Rect rect)
        {
            var path = AssetDatabase.GUIDToAssetPath(guid);
            if (!Statuses.TryGetValue(path, out var status)) return;
            var color = status.lock_state == "other" ? InUse
                : status.lock_state == "mine" ? CheckedOut
                : status.needs_sync ? NeedsSync
                : status.local_state == "untracked" ? Untracked
                : UpToDate;
            var dot = new Rect(rect.xMax - 9, rect.y + (rect.height - 6) * 0.5f, 6, 6);
            EditorGUI.DrawRect(dot, color);
        }
    }
}
