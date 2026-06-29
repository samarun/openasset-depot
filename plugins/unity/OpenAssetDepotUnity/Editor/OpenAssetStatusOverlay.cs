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

        private static void Draw(string guid, Rect rect)
        {
            var path = AssetDatabase.GUIDToAssetPath(guid);
            if (!Statuses.TryGetValue(path, out var status)) return;
            var color = status.lock_state == "other" ? new Color(0.82f, 0.35f, 0.31f)
                : status.lock_state == "mine" ? new Color(0.26f, 0.67f, 0.58f)
                : status.needs_sync ? new Color(0.86f, 0.65f, 0.24f)
                : new Color(0.43f, 0.61f, 0.78f);
            var dot = new Rect(rect.xMax - 9, rect.y + (rect.height - 6) * 0.5f, 6, 6);
            EditorGUI.DrawRect(dot, color);
        }
    }
}
