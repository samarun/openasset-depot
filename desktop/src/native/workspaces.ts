import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Workspace } from "../types/domain";

export interface NativeWorkspace {
  workspace_id: string;
  depot: string;
  stream: string;
  root: string;
  active_changelist_id?: string | null;
}

export interface RemoveWorkspaceResult {
  removed_metadata: boolean;
  removed_local_files: boolean;
  root: string;
}

export function isNativeDesktop(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function chooseWorkspaceDirectory(): Promise<string | undefined> {
  if (!isNativeDesktop()) return undefined;
  const selection = await open({
    directory: true,
    multiple: false,
    title: "Choose Workspace Folder",
  });
  return typeof selection === "string" ? selection : undefined;
}

export async function chooseWorkspaceFiles(root: string): Promise<string[]> {
  if (!isNativeDesktop()) return [];
  const selection = await open({
    directory: false,
    multiple: true,
    defaultPath: root,
    title: "Choose Assets to Add",
  });
  if (Array.isArray(selection)) return selection;
  return typeof selection === "string" ? [selection] : [];
}

export async function initializeLocalWorkspace(
  workspace: Workspace,
  depot: string,
  stream: string,
): Promise<NativeWorkspace | undefined> {
  if (!isNativeDesktop()) return undefined;
  return invoke<NativeWorkspace>("initialize_workspace", {
    request: {
      workspace_id: workspace.id,
      depot,
      stream,
      root: workspace.local_path,
    },
  });
}

export async function removeLocalWorkspace(
  workspace: Workspace,
  deleteLocalFiles: boolean,
): Promise<RemoveWorkspaceResult | undefined> {
  if (!isNativeDesktop()) return undefined;
  return invoke<RemoveWorkspaceResult>("remove_workspace_metadata", {
    request: {
      workspace_id: workspace.id,
      root: workspace.local_path,
      delete_local_files: deleteLocalFiles,
    },
  });
}
