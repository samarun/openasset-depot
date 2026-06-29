using System;
using System.Collections.Generic;
using System.IO;
using UnityEditor;
using UnityEngine;

namespace OpenAssetDepot.Unity
{
    internal sealed class OpenAssetAssetHooks : AssetModificationProcessor
    {
        private static readonly HashSet<string> LockRequiredExtensions = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
        {
            ".unity", ".prefab", ".asset", ".mat", ".controller", ".overridecontroller", ".playable"
        };

        private static string[] OnWillSaveAssets(string[] paths)
        {
            var allowed = new List<string>(paths.Length);
            var blocked = new List<string>();
            foreach (var path in paths)
            {
                if (!RequiresCheckout(path))
                {
                    allowed.Add(path);
                    continue;
                }
                try
                {
                    var status = OpenAssetCli.Status(path);
                    if (status.lock_state != "mine") OpenAssetCli.Checkout(path);
                    if (status.local_state == "untracked") AddMetaIfPresent(path);
                    allowed.Add(path);
                }
                catch (Exception error)
                {
                    blocked.Add(path + ": " + error.Message);
                }
            }
            if (blocked.Count > 0)
            {
                EditorUtility.DisplayDialog(
                    "OpenAsset Checkout Required",
                    "Unity did not save these assets because checkout failed:\n\n" + string.Join("\n", blocked),
                    "OK");
            }
            return allowed.ToArray();
        }

        private static AssetDeleteResult OnWillDeleteAsset(string assetPath, RemoveAssetOptions _options)
        {
            try
            {
                var status = OpenAssetCli.Status(assetPath);
                if (status.local_state == "untracked") return AssetDeleteResult.DidNotDelete;
                OpenAssetCli.Delete(assetPath);
                DeleteMetaIfTracked(assetPath);
                return AssetDeleteResult.DidNotDelete;
            }
            catch (Exception error)
            {
                Debug.LogError("OpenAsset blocked delete: " + error.Message);
                return AssetDeleteResult.FailedDelete;
            }
        }

        private static AssetMoveResult OnWillMoveAsset(string sourcePath, string destinationPath)
        {
            try
            {
                var status = OpenAssetCli.Status(sourcePath);
                if (status.local_state != "untracked")
                {
                    OpenAssetCli.Delete(sourcePath);
                    DeleteMetaIfTracked(sourcePath);
                }
                OpenAssetMoveQueue.Enqueue(destinationPath);
                return AssetMoveResult.DidNotMove;
            }
            catch (Exception error)
            {
                Debug.LogError("OpenAsset blocked move: " + error.Message);
                return AssetMoveResult.FailedMove;
            }
        }

        internal static bool RequiresCheckout(string path)
        {
            return LockRequiredExtensions.Contains(Path.GetExtension(path));
        }

        private static void AddMetaIfPresent(string assetPath)
        {
            var meta = assetPath + ".meta";
            if (!File.Exists(OpenAssetCli.ToAbsolutePath(meta))) return;
            var status = OpenAssetCli.Status(meta);
            if (status.local_state == "untracked") OpenAssetCli.Add(meta);
        }

        private static void DeleteMetaIfTracked(string assetPath)
        {
            var meta = assetPath + ".meta";
            if (!File.Exists(OpenAssetCli.ToAbsolutePath(meta))) return;
            var status = OpenAssetCli.Status(meta);
            if (status.local_state != "untracked") OpenAssetCli.Delete(meta);
        }
    }

    internal sealed class OpenAssetMovePostprocessor : AssetPostprocessor
    {
        private static void OnPostprocessAllAssets(
            string[] _imported,
            string[] _deleted,
            string[] moved,
            string[] _movedFrom)
        {
            foreach (var path in moved)
            {
                if (!OpenAssetMoveQueue.Consume(path)) continue;
                try
                {
                    OpenAssetCli.Add(path);
                    var meta = path + ".meta";
                    if (File.Exists(OpenAssetCli.ToAbsolutePath(meta))) OpenAssetCli.Add(meta);
                }
                catch (Exception error)
                {
                    Debug.LogError("OpenAsset could not add moved asset '" + path + "': " + error.Message);
                }
            }
        }
    }

    internal static class OpenAssetMoveQueue
    {
        private static readonly HashSet<string> Pending = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        internal static void Enqueue(string path) => Pending.Add(path);
        internal static bool Consume(string path) => Pending.Remove(path);
    }
}
