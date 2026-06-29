import type { AssetFile, LockInfo, SyncPlanEntry, Workspace } from "../types/domain";

const KIND_BY_EXTENSION: Record<string, string> = {
  blend: "Blender scene",
  fbx: "3D exchange",
  hip: "Houdini project",
  hipnc: "Houdini project",
  ma: "Maya scene",
  mb: "Maya scene",
  nk: "Nuke script",
  psd: "Photoshop document",
  uasset: "Unreal asset",
  umap: "Unreal map",
  unity: "Unity scene",
  wav: "Wave audio",
};

export function assetFromPendingPath(
  path: string,
  action: "add" | "edit" | "delete",
): AssetFile {
  const extension = extensionOf(path);
  const status = action === "add"
    ? "New File"
    : action === "delete"
      ? "Marked for Delete"
      : "Ready to Submit";
  return {
    id: `pending:${path}`,
    name: path.split("/").at(-1) ?? path,
    path,
    kind: KIND_BY_EXTENSION[extension] ?? `${extension.toUpperCase() || "Asset"} file`,
    size: action === "delete" ? "Delete" : "Pending",
    revision: 0,
    statuses: [status],
    previewTone: previewToneFor(extension),
    dependencies: [],
    source: "backend",
  };
}

export function assetsFromSyncPlan(
  entries: SyncPlanEntry[],
  locks: LockInfo[],
  workspace: Workspace,
  username: string,
  needsSyncPaths: ReadonlySet<string> = new Set(entries.map((entry) => entry.path)),
): AssetFile[] {
  const activeLocks = new Map(
    locks.filter((lock) => lock.state === "active").map((lock) => [lock.depot_path, lock]),
  );

  return entries.filter((entry) => !entry.deleted).map((entry) => {
    const extension = extensionOf(entry.path);
    const lock = activeLocks.get(entry.path);
    const statuses: AssetFile["statuses"] = [];

    if (lock) {
      statuses.push(lock.workspace_id === workspace.id ? "Checked Out" : "In Use");
    }
    statuses.push(needsSyncPaths.has(entry.path) ? "Needs Sync" : "Up to Date");

    return {
      id: entry.path,
      name: entry.path.split("/").at(-1) ?? entry.path,
      path: entry.path,
      kind: KIND_BY_EXTENSION[extension] ?? `${extension.toUpperCase() || "Asset"} file`,
      size: formatBytes(entry.size_bytes),
      revision: entry.revision_number,
      owner: lock ? (lock.workspace_id === workspace.id ? username : lock.user_id) : undefined,
      statuses,
      previewTone: previewToneFor(extension),
      dependencies: [],
      source: "backend",
    };
  });
}

export function lockBelongsToWorkspace(lock: LockInfo, workspace?: Workspace): boolean {
  return Boolean(workspace && lock.workspace_id === workspace.id);
}

function extensionOf(path: string): string {
  const fileName = path.split("/").at(-1) ?? path;
  const dot = fileName.lastIndexOf(".");
  return dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : "";
}

function previewToneFor(extension: string): AssetFile["previewTone"] {
  if (["fbx", "obj", "abc", "uasset"].includes(extension)) return "model";
  if (["wav", "mp3", "aif", "aiff"].includes(extension)) return "audio";
  if (["exr", "png", "jpg", "jpeg", "tif", "tiff", "psd"].includes(extension)) return "image";
  if (["umap", "unity", "ma", "mb", "blend", "hip", "hipnc"].includes(extension)) return "scene";
  return "document";
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1_024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1_024; index += 1) {
    value /= 1_024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}
