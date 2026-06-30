import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UserSession, Workspace } from "../types/domain";
import { createUuid } from "../utils/uuid";
import { isNativeDesktop } from "./workspaces";

export type IntegrationCommand =
  | "context"
  | "pending"
  | "status"
  | "checkout"
  | "add"
  | "delete"
  | "lock"
  | "unlock"
  | "revert"
  | "sync"
  | "submit"
  | "history"
  | "validate";

export interface IntegrationOptions {
  paths?: string[];
  reason?: string;
  adapter?: string;
  description?: string;
  onProgress?: (progress: IntegrationProgress) => void;
}

export interface IntegrationProgress {
  operationId: string;
  command: IntegrationCommand;
  phase: string;
  message: string;
  completed?: number;
  total?: number;
}

export interface PendingIntegrationFile {
  path: string;
  local_path: string;
  action: "add" | "edit" | "delete";
}

export interface PendingIntegrationResult {
  changelist_id?: string | null;
  files: PendingIntegrationFile[];
}

export async function runWorkspaceIntegration<T>(
  session: UserSession,
  workspace: Workspace,
  command: IntegrationCommand,
  options: IntegrationOptions = {},
): Promise<T> {
  if (!isNativeDesktop()) {
    return Promise.reject(new Error("This file operation requires the OpenAsset Depot desktop app."));
  }
  const operationId = createUuid();
  const unlisten = options.onProgress
    ? await listen<IntegrationProgress>("oad://operation-progress", (event) => {
        if (event.payload.operationId === operationId) options.onProgress?.(event.payload);
      })
    : undefined;
  try {
    return await invoke<T>("run_oad_integration", {
      request: {
        operationId,
        root: workspace.local_path,
        serverUrl: session.serverUrl,
        token: session.token,
        username: session.username,
        command,
        paths: options.paths ?? [],
        reason: options.reason,
        adapter: options.adapter,
        description: options.description,
      },
    });
  } finally {
    unlisten?.();
  }
}
