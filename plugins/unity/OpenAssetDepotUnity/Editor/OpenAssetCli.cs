using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading.Tasks;
using UnityEditor;
using UnityEngine;

namespace OpenAssetDepot.Unity
{
    internal static class OpenAssetCli
    {
        internal const int ProtocolVersion = 1;
        private const int MaxOutputCharacters = 4 * 1024 * 1024;

        internal static string ProjectRoot => Directory.GetParent(Application.dataPath).FullName;

        internal static Task<FileStatus> StatusAsync(string assetPath)
        {
            return Task.Run(() => Status(assetPath));
        }

        internal static FileStatus Status(string assetPath)
        {
            var json = Run(new[] { "integration", "status", ToAbsolutePath(assetPath) });
            var envelope = JsonUtility.FromJson<StatusEnvelope>(json);
            EnsureEnvelope(envelope != null ? envelope.protocol_version : 0, envelope != null && envelope.ok,
                envelope != null ? envelope.error : null);
            if (envelope.data == null || envelope.data.files == null || envelope.data.files.Length != 1)
                throw new OpenAssetException("OpenAsset CLI returned no status for the asset.");
            return envelope.data.files[0];
        }

        internal static Task<ActionData> CheckoutAsync(string assetPath)
        {
            return Task.Run(() => Action("checkout", assetPath, "--reason", "Editing in Unity"));
        }

        internal static ActionData Checkout(string assetPath)
        {
            return Action("checkout", assetPath, "--reason", "Editing in Unity");
        }

        internal static Task<ActionData> AddAsync(string assetPath)
        {
            return Task.Run(() => Action("add", assetPath));
        }

        internal static ActionData Add(string assetPath)
        {
            return Action("add", assetPath);
        }

        internal static ActionData Delete(string assetPath)
        {
            return Action("delete", assetPath, "--reason", "Deleting in Unity");
        }

        internal static Task<ActionData> RevertAsync(string assetPath)
        {
            return Task.Run(() => Action("revert", assetPath));
        }

        internal static Task<SyncData> SyncAsync()
        {
            return Task.Run(() =>
            {
                var envelope = JsonUtility.FromJson<SyncEnvelope>(Run(new[] { "integration", "sync" }, 30 * 60 * 1000));
                EnsureEnvelope(envelope != null ? envelope.protocol_version : 0, envelope != null && envelope.ok,
                    envelope != null ? envelope.error : null);
                return envelope.data ?? new SyncData { synced = Array.Empty<SyncEntry>() };
            });
        }

        internal static Task<SubmitData> SubmitAsync(string description)
        {
            return Task.Run(() =>
            {
                var envelope = JsonUtility.FromJson<SubmitEnvelope>(Run(
                    new[] { "integration", "submit", "--description", description }, 30 * 60 * 1000));
                EnsureEnvelope(envelope != null ? envelope.protocol_version : 0, envelope != null && envelope.ok,
                    envelope != null ? envelope.error : null);
                return envelope.data;
            });
        }

        internal static Task<ValidationData> ValidateAsync(IReadOnlyList<string> assetPaths)
        {
            return Task.Run(() =>
            {
                var arguments = new List<string> { "integration", "validate" };
                foreach (var path in assetPaths) arguments.Add(ToAbsolutePath(path));
                arguments.Add("--adapter");
                arguments.Add("Unity");
                var envelope = JsonUtility.FromJson<ValidationEnvelope>(Run(arguments));
                EnsureEnvelope(envelope != null ? envelope.protocol_version : 0, envelope != null && envelope.ok,
                    envelope != null ? envelope.error : null);
                return envelope.data;
            });
        }

        internal static string ToAbsolutePath(string assetPath)
        {
            if (Path.IsPathRooted(assetPath)) return Path.GetFullPath(assetPath);
            return Path.GetFullPath(Path.Combine(ProjectRoot, assetPath));
        }

        internal static string QuoteArgument(string value)
        {
            if (string.IsNullOrEmpty(value)) return "\"\"";
            var builder = new StringBuilder("\"");
            var backslashes = 0;
            foreach (var character in value)
            {
                if (character == '\\')
                {
                    backslashes++;
                    continue;
                }
                if (character == '"')
                {
                    builder.Append('\\', backslashes * 2 + 1);
                    builder.Append('"');
                    backslashes = 0;
                    continue;
                }
                builder.Append('\\', backslashes);
                backslashes = 0;
                builder.Append(character);
            }
            builder.Append('\\', backslashes * 2);
            builder.Append('"');
            return builder.ToString();
        }

        private static ActionData Action(string verb, string assetPath, params string[] extra)
        {
            var arguments = new List<string> { "integration", verb, ToAbsolutePath(assetPath) };
            arguments.AddRange(extra);
            var envelope = JsonUtility.FromJson<ActionEnvelope>(Run(arguments));
            EnsureEnvelope(envelope != null ? envelope.protocol_version : 0, envelope != null && envelope.ok,
                envelope != null ? envelope.error : null);
            return envelope.data;
        }

        private static string Run(IReadOnlyList<string> operationArguments, int timeoutMilliseconds = 120000)
        {
            var arguments = new List<string> { "--cwd", ProjectRoot };
            var server = OpenAssetSettings.ServerUrl;
            if (!string.IsNullOrWhiteSpace(server))
            {
                arguments.Add("--server");
                arguments.Add(server.Trim());
            }
            foreach (var argument in operationArguments) arguments.Add(argument);

            var startInfo = new ProcessStartInfo
            {
                FileName = OpenAssetSettings.CliPath,
                Arguments = string.Join(" ", arguments.ConvertAll(QuoteArgument)),
                WorkingDirectory = ProjectRoot,
                UseShellExecute = false,
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                CreateNoWindow = true,
            };
            using (var process = new Process { StartInfo = startInfo })
            {
                try
                {
                    if (!process.Start()) throw new OpenAssetException("Could not start the OpenAsset CLI.");
                }
                catch (Exception error)
                {
                    throw new OpenAssetException("Could not start the OpenAsset CLI: " + error.Message, error);
                }
                process.StandardInput.Close();
                var stdout = ReadBoundedAsync(process.StandardOutput);
                var stderr = ReadBoundedAsync(process.StandardError);
                if (!process.WaitForExit(timeoutMilliseconds))
                {
                    try { process.Kill(); } catch { }
                    throw new OpenAssetException("OpenAsset operation timed out.");
                }
                Task.WaitAll(stdout, stderr);
                if (stdout.Result.Exceeded || stderr.Result.Exceeded)
                    throw new OpenAssetException("OpenAsset CLI output exceeded the 4 MiB safety limit.");
                if (process.ExitCode != 0 && string.IsNullOrWhiteSpace(stdout.Result.Text))
                    throw new OpenAssetException(string.IsNullOrWhiteSpace(stderr.Result.Text)
                        ? "OpenAsset operation failed."
                        : stderr.Result.Text.Trim());
                return stdout.Result.Text;
            }
        }

        private static async Task<BoundedText> ReadBoundedAsync(StreamReader reader)
        {
            var buffer = new char[4096];
            var builder = new StringBuilder();
            var exceeded = false;
            int count;
            while ((count = await reader.ReadAsync(buffer, 0, buffer.Length)) > 0)
            {
                if (builder.Length + count <= MaxOutputCharacters) builder.Append(buffer, 0, count);
                else exceeded = true;
            }
            return new BoundedText(builder.ToString(), exceeded);
        }

        private static void EnsureEnvelope(int version, bool ok, string error)
        {
            if (version != ProtocolVersion)
                throw new OpenAssetException("The installed OpenAsset CLI protocol is not supported.");
            if (!ok) throw new OpenAssetException(string.IsNullOrWhiteSpace(error) ? "OpenAsset operation failed." : error);
        }

        private readonly struct BoundedText
        {
            internal readonly string Text;
            internal readonly bool Exceeded;
            internal BoundedText(string text, bool exceeded) { Text = text; Exceeded = exceeded; }
        }
    }

    internal sealed class OpenAssetException : Exception
    {
        internal OpenAssetException(string message) : base(message) { }
        internal OpenAssetException(string message, Exception inner) : base(message, inner) { }
    }
}
