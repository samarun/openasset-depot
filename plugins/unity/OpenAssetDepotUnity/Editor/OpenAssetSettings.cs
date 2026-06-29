using System;
using UnityEditor;
using UnityEngine;

namespace OpenAssetDepot.Unity
{
    internal static class OpenAssetSettings
    {
        private const string CliKey = "OpenAssetDepot.CliPath";
        private const string ServerKey = "OpenAssetDepot.ServerUrl";

        internal static string CliPath
        {
            get
            {
                var configured = EditorPrefs.GetString(CliKey, string.Empty);
                if (!string.IsNullOrWhiteSpace(configured)) return configured;
                var environment = Environment.GetEnvironmentVariable("OAD_CLI");
                return string.IsNullOrWhiteSpace(environment) ? "oad" : environment;
            }
            set => EditorPrefs.SetString(CliKey, value ?? string.Empty);
        }

        internal static string ServerUrl
        {
            get => EditorPrefs.GetString(ServerKey, string.Empty);
            set => EditorPrefs.SetString(ServerKey, value ?? string.Empty);
        }

        [SettingsProvider]
        internal static SettingsProvider CreateProvider()
        {
            return new SettingsProvider("Project/OpenAsset Depot", SettingsScope.Project)
            {
                label = "OpenAsset Depot",
                guiHandler = _ =>
                {
                    EditorGUILayout.LabelField("CLI Connection", EditorStyles.boldLabel);
                    CliPath = EditorGUILayout.TextField("oad CLI", CliPath);
                    ServerUrl = EditorGUILayout.TextField("Server URL Override", ServerUrl);
                    EditorGUILayout.HelpBox(
                        "Authentication is managed by 'oad login'. Tokens are not stored in Unity project settings.",
                        MessageType.Info);
                },
                keywords = new[] { "OpenAsset", "Depot", "source control", "version control" },
            };
        }
    }
}
