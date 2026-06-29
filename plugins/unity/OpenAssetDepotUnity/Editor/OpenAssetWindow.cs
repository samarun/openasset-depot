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
            window.titleContent = new GUIContent("OpenAsset");
            window.minSize = new Vector2(360, 390);
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
            EditorGUILayout.LabelField("OPENASSET DEPOT", EditorStyles.miniBoldLabel);
            EditorGUILayout.LabelField(_assetPath ?? "No asset selected", EditorStyles.boldLabel);
            EditorGUILayout.HelpBox(_status, MessageType.None);

            using (new EditorGUI.DisabledScope(_busy || string.IsNullOrEmpty(_assetPath)))
            {
                using (new EditorGUILayout.HorizontalScope())
                {
                    if (GUILayout.Button("Refresh Status", GUILayout.Height(28))) RunStatus();
                    if (GUILayout.Button("Check Out", GUILayout.Height(28))) RunCheckout();
                }
                if (GUILayout.Button("Add Asset + .meta", GUILayout.Height(28))) RunAdd();
                if (GUILayout.Button("Validate Asset", GUILayout.Height(28))) RunValidate();
                if (GUILayout.Button("Revert Checkout", GUILayout.Height(28))) RunRevert();
            }
            EditorGUILayout.Space(8);
            using (new EditorGUI.DisabledScope(_busy))
            {
                if (GUILayout.Button("Sync Latest", GUILayout.Height(30))) RunSync();
                _description = EditorGUILayout.TextField("Description", _description);
                if (GUILayout.Button("Submit Changes", GUILayout.Height(32))) RunSubmit();
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
            if (!EditorUtility.DisplayDialog("Revert Checkout",
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

        private static string FormatStatus(FileStatus status)
        {
            var values = new List<string> { ObjectNames.NicifyVariableName(status.local_state) };
            if (status.needs_sync) values.Add("Needs Sync");
            if (status.lock_state == "mine") values.Add("Checked Out by Me");
            else if (status.lock_state == "other") values.Add("Checked Out Elsewhere");
            return string.Join(" | ", values);
        }

        private static int Count(Array values) => values != null ? values.Length : 0;
    }
}
