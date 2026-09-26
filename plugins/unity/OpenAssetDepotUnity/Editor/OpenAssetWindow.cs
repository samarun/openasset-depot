using System;
using System.Collections.Generic;
using System.IO;
using System.Threading.Tasks;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace OpenAssetDepot.Unity
{
    internal sealed class OpenAssetWindow : EditorWindow
    {
        private bool _busy;
        private string _status = "Select an asset or save the current scene to begin.";
        private string _description = "Unity asset update";
        private string _assetPath;

        [MenuItem("Window/OpenAsset Depot")]
        internal static void ShowWindow()
        {
            var window = GetWindow<OpenAssetWindow>();
            window.titleContent = new GUIContent(OpenAssetWords.ProductName);
            window.minSize = new Vector2(360, 460);
            window.Show();
        }

        private void OnEnable()
        {
            Selection.selectionChanged += SelectionChanged;
            SelectionChanged();
        }

        private void OnDisable()
        {
            Selection.selectionChanged -= SelectionChanged;
        }

        private void OnGUI()
        {
            EditorGUILayout.Space(8);
            EditorGUILayout.LabelField(OpenAssetWords.ProductName, EditorStyles.miniBoldLabel);
            EditorGUILayout.LabelField(_assetPath ?? "No asset selected", EditorStyles.boldLabel);
            EditorGUILayout.HelpBox(_status, MessageType.None);

            using (new EditorGUI.DisabledScope(_busy || string.IsNullOrEmpty(_assetPath)))
            {
                using (new EditorGUILayout.HorizontalScope())
                {
                    if (GUILayout.Button(OpenAssetWords.Refresh, GUILayout.Height(28))) RunStatus();
                    if (GUILayout.Button(OpenAssetWords.Checkout, GUILayout.Height(28))) RunCheckout();
                }
                if (GUILayout.Button(OpenAssetWords.Add, GUILayout.Height(28))) RunAdd();
                if (GUILayout.Button(OpenAssetWords.Validate, GUILayout.Height(28))) RunValidate();
                if (GUILayout.Button(OpenAssetWords.Revert, GUILayout.Height(28))) RunRevert();
            }
            EditorGUILayout.Space(8);
            using (new EditorGUI.DisabledScope(_busy))
            {
                if (GUILayout.Button(OpenAssetWords.Sync, GUILayout.Height(30))) RunSync();
                _description = EditorGUILayout.TextField(OpenAssetWords.DescriptionField, _description);
                if (GUILayout.Button(OpenAssetWords.Submit, GUILayout.Height(32))) RunSubmit();
                using (new EditorGUILayout.HorizontalScope())
                {
                    if (GUILayout.Button(OpenAssetWords.Shelve, GUILayout.Height(28))) RunShelve();
                    if (GUILayout.Button(OpenAssetWords.Unshelve, GUILayout.Height(28))) RunUnshelve();
                }
            }
            EditorGUILayout.Space(8);
            if (GUILayout.Button("Open Settings")) SettingsService.OpenProjectSettings("Project/OpenAsset Depot");
        }

        private void SelectionChanged()
        {
            _assetPath = SelectedAssetPath();
            Repaint();
            if (!string.IsNullOrEmpty(_assetPath)) RunStatus();
        }

        private async void RunStatus()
        {
            await Run(async () =>
            {
                var state = await OpenAssetCli.StatusAsync(_assetPath);
                OpenAssetStatusCache.Set(_assetPath, state);
                return FormatStatus(state);
            });
        }

        private async void RunCheckout()
        {
            await Run(async () => "Checked out " + (await OpenAssetCli.CheckoutAsync(_assetPath)).path);
        }

        private async void RunAdd()
        {
            await Run(async () =>
            {
                await OpenAssetCli.AddAsync(_assetPath);
                var meta = _assetPath + ".meta";
                if (File.Exists(OpenAssetCli.ToAbsolutePath(meta))) await OpenAssetCli.AddAsync(meta);
                return "Added asset and metadata to the pending changelist.";
            });
        }

        private async void RunValidate()
        {
            await Run(async () =>
            {
                var paths = new List<string> { _assetPath };
                if (File.Exists(OpenAssetCli.ToAbsolutePath(_assetPath + ".meta"))) paths.Add(_assetPath + ".meta");
                var result = await OpenAssetCli.ValidateAsync(paths);
                var errors = Count(result.adapter != null ? result.adapter.errors : null) +
                             Count(result.core != null ? result.core.errors : null);
                var warnings = Count(result.adapter != null ? result.adapter.warnings : null) +
                               Count(result.core != null ? result.core.warnings : null);
                return $"Validation: {errors} error(s), {warnings} warning(s)";
            });
        }

        private async void RunRevert()
        {
            if (!EditorUtility.DisplayDialog(OpenAssetWords.Revert,
                    "Remove this asset from the pending changelist and release its lock? The local file is preserved.",
                    "Revert", "Cancel")) return;
            await Run(async () => "Reverted checkout for " + (await OpenAssetCli.RevertAsync(_assetPath)).path);
        }

        private async void RunSync()
        {
            await Run(async () =>
            {
                var result = await OpenAssetCli.SyncAsync();
                await Task.Yield();
                AssetDatabase.Refresh(ImportAssetOptions.ForceUpdate);
                return $"Synced {(result.synced != null ? result.synced.Length : 0)} file(s).";
            });
        }

        private async void RunSubmit()
        {
            if (string.IsNullOrWhiteSpace(_description))
            {
                _status = "Enter a submit description.";
                return;
            }
            await Run(async () =>
            {
                var result = await OpenAssetCli.SubmitAsync(_description.Trim());
                return $"Submitted {(result.revisions != null ? result.revisions.Length : 0)} file(s).";
            });
        }

        private async void RunShelve()
        {
            await Run(async () =>
            {
                var result = await OpenAssetCli.ShelveAsync();
                return $"Shelved {(result.files != null ? result.files.Length : 0)} file(s)";
            });
        }

        private async void RunUnshelve()
        {
            await Run(async () =>
            {
                var result = await OpenAssetCli.UnshelveAsync();
                await Task.Yield();
                AssetDatabase.Refresh(ImportAssetOptions.ForceUpdate);
                return $"Restored {result.restored_count} file(s).";
            });
        }

        private async Task Run(Func<Task<string>> operation)
        {
            if (_busy) return;
            _busy = true;
            _status = "Working...";
            Repaint();
            try { _status = await operation(); }
            catch (Exception error) { _status = error.Message; }
            finally { _busy = false; Repaint(); }
        }

        private static string SelectedAssetPath()
        {
            var selected = Selection.activeObject != null ? AssetDatabase.GetAssetPath(Selection.activeObject) : null;
            if (!string.IsNullOrEmpty(selected) && !AssetDatabase.IsValidFolder(selected)) return selected;
            var scene = SceneManager.GetActiveScene();
            return scene.IsValid() && !string.IsNullOrEmpty(scene.path) ? scene.path : null;
        }

        // Mirrors status_label in plugins/common/openasset_depot_bridge/words.py,
        // including its precedence: another artist's lock outranks the caller's
        // own pending edit because it is the part they cannot resolve alone.
        private static string FormatStatus(FileStatus status)
        {
            if (status.lock_state == "other") return OpenAssetWords.InUse;
            if (status.pending_action == "delete") return OpenAssetWords.MarkedForDelete;
            if (status.pending_action == "add" || status.local_state == "untracked") return OpenAssetWords.NewFile;
            if (!string.IsNullOrEmpty(status.pending_action)) return OpenAssetWords.ReadyToSubmit;
            if (status.lock_state == "mine") return OpenAssetWords.CheckedOut;
            return status.needs_sync ? OpenAssetWords.NeedsSync : OpenAssetWords.UpToDate;
        }

        private static int Count(Array values) => values != null ? values.Length : 0;
    }
}
