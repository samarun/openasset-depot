import { useCallback, useEffect, useMemo, useState } from "react";
import {
  type BrowserUpload,
  OpenAssetApiClient,
  friendlyApiError,
  isConnectionError,
} from "./api/client";
import { ActionNotice } from "./components/ActionNotice";
import { CommandPalette, type CommandAction } from "./components/CommandPalette";
import { ConnectionStatusBar } from "./components/ConnectionStatusBar";
import { OperationProgressBar } from "./components/OperationProgressBar";
import { ReviewViewer, type ReviewCommentDraft } from "./components/ReviewViewer";
import { Sidebar } from "./components/Sidebar";
import { SubmitDialog } from "./components/SubmitDialog";
import { TopBar } from "./components/TopBar";
import { assetFromPendingPath, assetsFromSyncPlan } from "./data/assets";
import {
  type PendingIntegrationResult,
  type IntegrationProgress,
  runWorkspaceIntegration,
} from "./native/integration";
import {
  clearCliSession,
  chooseWorkspaceFiles,
  configureCliSession,
  initializeLocalWorkspace,
  isNativeDesktop,
  removeLocalWorkspace,
} from "./native/workspaces";
import {
  mockChangelists,
  mockDepots,
  mockFiles,
  mockFiletypes,
  mockLocks,
  mockWorkspace,
} from "./data/mockData";
import { Admin } from "./pages/Admin";
import { Changes } from "./pages/Changes";
import { History } from "./pages/History";
import { Home } from "./pages/Home";
import { Locks } from "./pages/Locks";
import { Login } from "./pages/Login";
import { Settings } from "./pages/Settings";
import { Workspace } from "./pages/Workspace";
import { WorkspaceSelector } from "./pages/WorkspaceSelector";
import type {
  AssetFile,
  Changelist,
  CreateWorkspaceInput,
  Depot,
  FileHistoryEntry,
  FileTypeRule,
  LockInfo,
  ReviewComment,
  ReviewMedia,
  Stream,
  SyncPlanEntry,
  UserSession,
  ValidationMessage,
  ViewKey,
  Workspace as WorkspaceModel,
} from "./types/domain";
import { createUuid } from "./utils/uuid";

type Theme = "light" | "dark";
type ConnectionMode = "connecting" | "connected" | "demo" | "reconnecting";
type Notice = { message: string; tone: "success" | "error" | "info" };
const DIRECT_REVIEW_EXTENSIONS = new Set([
  "png", "jpg", "jpeg", "webp", "gif",
  "mp4", "m4v", "webm", "mov",
  "mp3", "wav", "ogg", "oga",
  "fbx", "glb", "gltf",
]);

export function App() {
  const [session, setSession] = useState<UserSession | undefined>(() => readStoredSession());
  const [theme, setTheme] = useState<Theme>(() => readStoredTheme());
  const [connectionMode, setConnectionMode] = useState<ConnectionMode>(() =>
    session?.token === "mock-preview-token" ? "demo" : "connecting",
  );
  const [activeView, setActiveView] = useState<ViewKey>("home");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [workspaces, setWorkspaces] = useState<WorkspaceModel[]>([]);
  const [selectedWorkspace, setSelectedWorkspace] = useState<WorkspaceModel | undefined>();
  const [depots, setDepots] = useState<Depot[]>([]);
  const [streams, setStreams] = useState<Stream[]>([]);
  const [locks, setLocks] = useState<LockInfo[]>([]);
  const [filetypes, setFiletypes] = useState<FileTypeRule[]>([]);
  const [syncPlan, setSyncPlan] = useState<SyncPlanEntry[]>([]);
  const [workspaceFiles, setWorkspaceFiles] = useState<SyncPlanEntry[]>([]);
  const [workspaceFilesHasMore, setWorkspaceFilesHasMore] = useState(false);
  const [pendingFiles, setPendingFiles] = useState<PendingIntegrationResult["files"]>([]);
  const [browserUploads, setBrowserUploads] = useState<BrowserUpload[]>([]);
  const [browserChangelistId, setBrowserChangelistId] = useState<string>();
  const [selectedFileId, setSelectedFileId] = useState<string>();
  const [historyEntries, setHistoryEntries] = useState<FileHistoryEntry[]>([]);
  const [historyError, setHistoryError] = useState<string>();
  const [historyLoading, setHistoryLoading] = useState(false);
  const [submitOpen, setSubmitOpen] = useState(false);
  const [activeChangelist, setActiveChangelist] = useState<Changelist>(mockChangelists[0]);
  const [validationWarnings, setValidationWarnings] = useState<ValidationMessage[]>([]);
  const [validationErrors, setValidationErrors] = useState<ValidationMessage[]>([]);
  const [validating, setValidating] = useState(false);
  const [loadingWorkspaces, setLoadingWorkspaces] = useState(false);
  const [loadingLocks, setLoadingLocks] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [workspaceError, setWorkspaceError] = useState<string>();
  const [notice, setNotice] = useState<Notice>();
  const [operationProgress, setOperationProgress] = useState<IntegrationProgress>();
  const [reviewFile, setReviewFile] = useState<AssetFile>();
  const [demoReviewComments, setDemoReviewComments] = useState<ReviewComment[]>([]);

  const api = useMemo(
    () => (session ? new OpenAssetApiClient(session.serverUrl) : undefined),
    [session],
  );
  useEffect(() => {
    if (!session || session.token === "mock-preview-token" || !isNativeDesktop()) return;
    void configureCliSession(session).catch((error: unknown) => {
      setNotice({
        message: `Signed in, but creative plug-ins could not be connected automatically: ${friendlyApiError(error)}`,
        tone: "error",
      });
    });
  }, [session]);
  const logout = useCallback(() => {
    void clearCliSession();
    sessionStorage.removeItem("oad.session");
    setSession(undefined);
    setSelectedWorkspace(undefined);
  }, []);
  const demoMode = connectionMode === "demo";
  const files = useMemo(() => {
    if (demoMode) return mockFiles;
    if (!selectedWorkspace || !session) return [];
    const needsSync = new Set(syncPlan.map((entry) => entry.path));
    const remote = assetsFromSyncPlan(
      workspaceFiles,
      locks,
      selectedWorkspace,
      session.username,
      needsSync,
    );
    const byPath = new Map(remote.map((file) => [file.path, file]));
    for (const pending of pendingFiles) {
      const existing = byPath.get(pending.path);
      if (existing) {
        const status = pending.action === "add"
          ? "New File"
          : pending.action === "delete"
            ? "Marked for Delete"
            : "Ready to Submit";
        const pendingStatuses = new Set(["New File", "Ready to Submit", "Marked for Delete"]);
        existing.statuses = [status, ...existing.statuses.filter((value) => !pendingStatuses.has(value))];
      } else {
        byPath.set(pending.path, assetFromPendingPath(pending.path, pending.action));
      }
    }
    return Array.from(byPath.values());
  }, [demoMode, locks, pendingFiles, selectedWorkspace, session, syncPlan, workspaceFiles]);
  const liveChangelist = useMemo<Changelist | undefined>(() => {
    if (pendingFiles.length === 0) return undefined;
    return {
      id: "workspace-pending",
      title: "Workspace Changes",
      description: "Creative changes from this workspace",
      files: pendingFiles.map((file) => assetFromPendingPath(file.path, file.action)),
      warnings: [],
      ready: true,
      source: "backend",
    };
  }, [pendingFiles]);
  const changelists = demoMode ? mockChangelists : liveChangelist ? [liveChangelist] : [];
  const selectedFile = files.find((file) => file.id === selectedFileId) ?? files[0];
  const loadAssetPreview = useCallback(async (file: AssetFile) => {
    if (!session || !api || !selectedWorkspace || demoMode || !file.previewAvailable) return undefined;
    const blob = await api.downloadPreview(
      session.token,
      selectedWorkspace.id,
      file.path,
      file.revision,
    );
    return blob ? URL.createObjectURL(blob) : undefined;
  }, [api, demoMode, selectedWorkspace, session]);

  const loadReviewMedia = useCallback(async (file: AssetFile): Promise<ReviewMedia | undefined> => {
    if (demoMode) {
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#173c37"/><stop offset="1" stop-color="#11181b"/></linearGradient></defs><rect width="100%" height="100%" fill="url(#g)"/><circle cx="960" cy="210" r="220" fill="#76d2c2" opacity=".12"/><text x="72" y="610" fill="#dce9e6" font-family="system-ui" font-size="48" font-weight="700">${escapeSvg(file.name)}</text><text x="75" y="658" fill="#8eaaa5" font-family="system-ui" font-size="22">Rendered DCC review proxy · Version ${file.revision}</text></svg>`;
      return {
        blob: new Blob([svg], { type: "image/svg+xml" }),
        contentType: "image/svg+xml",
        source: "preview",
      };
    }
    if (!session || !api || !selectedWorkspace) return undefined;
    const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
    if (file.reviewProxyAvailable) {
      const proxy = await api.downloadReviewProxy(session.token, selectedWorkspace.id, file.path, file.revision);
      if (proxy) return { blob: proxy, contentType: proxy.type || "application/octet-stream", source: "asset" };
    }
    if (DIRECT_REVIEW_EXTENSIONS.has(extension)) {
      const blob = await api.downloadReviewMedia(session.token, selectedWorkspace.id, file.path, file.revision);
      if (blob) return { blob, contentType: blob.type || "application/octet-stream", source: "asset" };
    }
    const preview = await api.downloadPreview(session.token, selectedWorkspace.id, file.path, file.revision);
    return preview ? { blob: preview, contentType: preview.type, source: "preview" } : undefined;
  }, [api, demoMode, selectedWorkspace, session]);

  const loadReviewComments = useCallback(async (file: AssetFile): Promise<ReviewComment[]> => {
    if (demoMode) {
      return demoReviewComments.filter((comment) =>
        comment.path === file.path && comment.revision_number === file.revision
      );
    }
    if (!session || !api || !selectedWorkspace) return [];
    return api.listReviewComments(session.token, selectedWorkspace.id, file.path, file.revision);
  }, [api, demoMode, demoReviewComments, selectedWorkspace, session]);

  const createReviewComment = useCallback(async (
    file: AssetFile,
    draft: ReviewCommentDraft,
  ): Promise<ReviewComment> => {
    if (demoMode) {
      const created: ReviewComment = {
        id: createUuid(),
        path: file.path,
        revision_number: file.revision,
        author_user_id: session?.username ?? "demo.artist",
        author: session?.username ?? "demo.artist",
        body: draft.body,
        timecode_ms: draft.timecodeMs,
        frame_number: draft.frameNumber,
        annotation: draft.annotation,
        created_at: new Date().toISOString(),
      };
      setDemoReviewComments((current) => [...current, created]);
      return created;
    }
    if (!session || !api || !selectedWorkspace) throw new Error("Connect to the depot before commenting.");
    return api.createReviewComment(session.token, {
      workspace_id: selectedWorkspace.id,
      path: file.path,
      revision_number: file.revision,
      body: draft.body,
      timecode_ms: draft.timecodeMs,
      frame_number: draft.frameNumber,
      annotation: draft.annotation,
    });
  }, [api, demoMode, selectedWorkspace, session]);

  const resolveReviewComment = useCallback(async (
    comment: ReviewComment,
    resolved: boolean,
  ): Promise<ReviewComment> => {
    if (demoMode) {
      const updated: ReviewComment = {
        ...comment,
        resolved_at: resolved ? new Date().toISOString() : null,
        resolved_by: resolved ? (session?.username ?? "demo.artist") : null,
      };
      setDemoReviewComments((current) => current.map((item) => item.id === comment.id ? updated : item));
      return updated;
    }
    if (!session || !api || !selectedWorkspace) throw new Error("Connect to the depot before updating comments.");
    return api.resolveReviewComment(session.token, selectedWorkspace.id, comment.id, resolved);
  }, [api, demoMode, selectedWorkspace, session]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("oad.theme", theme);
  }, [theme]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen(true);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const activateDemoMode = useCallback(() => {
    setConnectionMode("demo");
    setWorkspaces([mockWorkspace]);
    setDepots(mockDepots);
    setStreams([]);
    setLocks(mockLocks);
    setFiletypes(mockFiletypes);
    setSyncPlan([]);
    setSelectedWorkspace(mockWorkspace);
    setSelectedFileId(mockFiles[0].id);
    setWorkspaceError(undefined);
  }, []);

  const refreshBackendData = useCallback(async () => {
    if (!session || !api) return;
    if (session.token === "mock-preview-token") {
      activateDemoMode();
      return;
    }

    setLoadingWorkspaces(true);
    setWorkspaceError(undefined);
    try {
      const [nextWorkspaces, nextDepots, nextStreams, nextLocks, nextFiletypes] = await Promise.all([
        api.listWorkspaces(session.token),
        api.listDepots(session.token),
        api.listStreams(session.token),
        api.listLocks(session.token),
        api.listFiletypes(session.token),
      ]);
      setConnectionMode("connected");
      setWorkspaces(nextWorkspaces);
      setDepots(nextDepots);
      setStreams(nextStreams);
      setLocks(nextLocks.map((lock) => ({ ...lock, source: "backend" })));
      setFiletypes(nextFiletypes.map((rule) => ({ ...rule, source: "backend" })));
      setSelectedWorkspace((current) =>
        nextWorkspaces.find((workspace) => workspace.id === current?.id) ?? nextWorkspaces[0],
      );
    } catch (error) {
      if (isConnectionError(error)) {
        activateDemoMode();
      } else {
        setConnectionMode("connected");
        setWorkspaces([]);
        setDepots([]);
        setStreams([]);
        setLocks([]);
        setFiletypes([]);
        setWorkspaceError(friendlyApiError(error));
      }
    } finally {
      setLoadingWorkspaces(false);
    }
  }, [activateDemoMode, api, session]);

  useEffect(() => {
    void refreshBackendData();
  }, [refreshBackendData]);

  const refreshSyncPlan = useCallback(async (announce: boolean) => {
    if (!session || !api || !selectedWorkspace || demoMode) return;
    setActionBusy(true);
    try {
      const nextPlan = await api.planSync(session.token, selectedWorkspace.id);
      setSyncPlan(nextPlan);
      if (announce) {
        setNotice({
          message: nextPlan.length > 0
            ? `${nextPlan.length} ${nextPlan.length === 1 ? "file is" : "files are"} ready to sync.`
            : "Workspace is already up to date.",
          tone: "success",
        });
      }
    } catch (error) {
      if (isConnectionError(error)) activateDemoMode();
      setNotice({ message: friendlyApiError(error), tone: "error" });
    } finally {
      setActionBusy(false);
    }
  }, [activateDemoMode, api, demoMode, selectedWorkspace, session]);

  const refreshWorkspaceFiles = useCallback(async () => {
    if (!session || !api || !selectedWorkspace || demoMode) return;
    try {
      const entries = await api.listWorkspaceFiles(session.token, selectedWorkspace.id);
      setWorkspaceFiles(entries);
      setWorkspaceFilesHasMore(entries.length === 1_000);
    } catch (error) {
      if (isConnectionError(error)) activateDemoMode();
      setNotice({ message: friendlyApiError(error), tone: "error" });
    }
  }, [activateDemoMode, api, demoMode, selectedWorkspace, session]);

  const loadMoreWorkspaceFiles = useCallback(async () => {
    if (!session || !api || !selectedWorkspace || demoMode || !workspaceFilesHasMore) return;
    const afterPath = workspaceFiles.at(-1)?.path;
    if (!afterPath) return;
    setActionBusy(true);
    try {
      const entries = await api.listWorkspaceFiles(session.token, selectedWorkspace.id, afterPath);
      setWorkspaceFiles((current) => [...current, ...entries]);
      setWorkspaceFilesHasMore(entries.length === 1_000);
    } catch (error) {
      setNotice({ message: friendlyApiError(error), tone: "error" });
    } finally {
      setActionBusy(false);
    }
  }, [api, demoMode, selectedWorkspace, session, workspaceFiles, workspaceFilesHasMore]);

  const refreshPending = useCallback(async () => {
    if (!session || !selectedWorkspace || demoMode || !isNativeDesktop()) return;
    try {
      const result = await runWorkspaceIntegration<PendingIntegrationResult>(
        session,
        selectedWorkspace,
        "pending",
      );
      setPendingFiles(result.files);
    } catch (error) {
      setNotice({ message: friendlyApiError(error), tone: "error" });
    }
  }, [demoMode, selectedWorkspace, session]);

  useEffect(() => {
    setSyncPlan([]);
    setWorkspaceFiles([]);
    setWorkspaceFilesHasMore(false);
    setPendingFiles([]);
    setBrowserUploads([]);
    setBrowserChangelistId(undefined);
  }, [selectedWorkspace?.id]);

  useEffect(() => {
    if (connectionMode === "connected" && selectedWorkspace) {
      void refreshSyncPlan(false);
      void refreshWorkspaceFiles();
      void refreshPending();
    }
  }, [connectionMode, refreshPending, refreshSyncPlan, refreshWorkspaceFiles, selectedWorkspace]);

  const syncLatest = useCallback(async () => {
    if (!session || !selectedWorkspace || demoMode) {
      await refreshSyncPlan(true);
      return;
    }
    if (!isNativeDesktop()) {
      setActionBusy(true);
      try {
        await Promise.all([refreshSyncPlan(false), refreshWorkspaceFiles()]);
        setNotice({
          message: "Depot refreshed. Desktop and DCC workspaces can use Sync Latest to download these revisions.",
          tone: "success",
        });
      } finally {
        setActionBusy(false);
      }
      return;
    }
    setActionBusy(true);
    try {
      const result = await runWorkspaceIntegration<{ synced_count: number }>(
        session,
        selectedWorkspace,
        "sync",
        { onProgress: setOperationProgress },
      );
      setNotice({
        message: result.synced_count > 0
          ? `Synced ${result.synced_count} ${result.synced_count === 1 ? "file" : "files"}.`
          : "Workspace is already up to date.",
        tone: "success",
      });
      await refreshSyncPlan(false);
      await refreshWorkspaceFiles();
    } catch (error) {
      setNotice({ message: friendlyApiError(error), tone: "error" });
    } finally {
      setActionBusy(false);
      window.setTimeout(() => setOperationProgress(undefined), 900);
    }
  }, [demoMode, refreshSyncPlan, refreshWorkspaceFiles, selectedWorkspace, session]);

  const refreshLocks = useCallback(async () => {
    if (!session || !api) return;
    if (demoMode) {
      setLocks(mockLocks);
      return;
    }
    setLoadingLocks(true);
    try {
      const nextLocks = await api.listLocks(session.token);
      setLocks(nextLocks.map((lock) => ({ ...lock, source: "backend" })));
    } catch (error) {
      if (isConnectionError(error)) activateDemoMode();
      setNotice({ message: friendlyApiError(error), tone: "error" });
    } finally {
      setLoadingLocks(false);
    }
  }, [activateDemoMode, api, demoMode, session]);

  const retryConnection = useCallback(async () => {
    if (!session || !api) return;
    if (session.token !== "mock-preview-token") {
      setConnectionMode("reconnecting");
      await refreshBackendData();
      return;
    }
    setActionBusy(true);
    setConnectionMode("reconnecting");
    try {
      await api.ready();
      setNotice({ message: "Server is reachable. Sign in from Settings to use live data.", tone: "info" });
    } catch (error) {
      setConnectionMode("demo");
      setNotice({ message: friendlyApiError(error), tone: "error" });
    } finally {
      setActionBusy(false);
    }
  }, [api, refreshBackendData, session]);

  const createWorkspace = useCallback(async (input: CreateWorkspaceInput) => {
    if (!session || !api || demoMode) {
      throw new Error("Connect to the server before creating a workspace.");
    }
    const workspace = await api.createWorkspace(session.token, input);
    try {
      await initializeLocalWorkspace(workspace, input.depot, input.stream);
    } catch (error) {
      await api.deleteWorkspace(session.token, workspace.id).catch(() => undefined);
      throw error;
    }
    setWorkspaces((current) => [...current, workspace].sort((a, b) => a.name.localeCompare(b.name)));
    setSelectedWorkspace(workspace);
    setNotice({ message: `Workspace ${workspace.name} is ready.`, tone: "success" });
  }, [api, demoMode, session]);

  const deleteWorkspace = useCallback(async (deleteLocalFiles: boolean) => {
    if (!session || !api || !selectedWorkspace || demoMode) return;
    setActionBusy(true);
    const workspace = selectedWorkspace;
    try {
      const response = await api.deleteWorkspace(session.token, workspace.id);
      await removeLocalWorkspace(workspace, deleteLocalFiles);
      setWorkspaces((current) => current.filter((candidate) => candidate.id !== workspace.id));
      setSelectedWorkspace(undefined);
      setNotice({
        message: `Workspace removed. ${response.released_locks} lock(s) released.`,
        tone: "success",
      });
    } catch (error) {
      setNotice({ message: friendlyApiError(error), tone: "error" });
      throw error;
    } finally {
      setActionBusy(false);
    }
  }, [api, demoMode, selectedWorkspace, session]);

  const createDepot = useCallback(async (name: string, description?: string) => {
    if (!session || !api || demoMode) throw new Error("Connect to the server before creating a depot.");
    const depot = await api.createDepot(session.token, name, description);
    setDepots((current) => [...current, depot].sort((a, b) => a.name.localeCompare(b.name)));
    setNotice({ message: `Depot ${depot.name} created.`, tone: "success" });
    return depot;
  }, [api, demoMode, session]);

  const createStream = useCallback(async (name: string, depotId: string) => {
    if (!session || !api || demoMode) throw new Error("Connect to the server before creating a stream.");
    const stream = await api.createStream(session.token, name, depotId);
    setStreams((current) => [...current, stream].sort((a, b) => a.name.localeCompare(b.name)));
    setNotice({ message: `Stream ${stream.name} created.`, tone: "success" });
    return stream;
  }, [api, demoMode, session]);

  const validateSubmit = useCallback(async () => {
    if (!session || !api || !selectedWorkspace || demoMode) {
      setValidationWarnings(activeChangelist.warnings);
      setValidationErrors([]);
      return true;
    }
    setValidating(true);
    try {
      if (isNativeDesktop()) {
        const response = await runWorkspaceIntegration<{
          adapter: { warnings: ValidationMessage[]; errors: ValidationMessage[] };
          core: { warnings: ValidationMessage[]; errors: ValidationMessage[] };
        }>(session, selectedWorkspace, "validate", {
          paths: activeChangelist.files.map((file) => file.path),
          onProgress: setOperationProgress,
        });
        setValidationWarnings([...response.adapter.warnings, ...response.core.warnings]);
        const errors = [...response.adapter.errors, ...response.core.errors];
        setValidationErrors(errors);
        if (errors.length > 0) {
          setNotice({ message: "Resolve validation errors before submitting.", tone: "error" });
          return false;
        }
        return true;
      }
      const response = await api.validateWorkspace(
        session.token,
        selectedWorkspace.id,
        browserUploads.length > 0
          ? browserUploads.map((upload) => ({ path: upload.path, size_bytes: upload.file.size }))
          : activeChangelist.files.map((file) => ({ path: file.path, size_bytes: 1 })),
      );
      setValidationWarnings(response.warnings);
      setValidationErrors(response.errors);
      if (response.errors.length > 0) {
        setNotice({ message: "Resolve validation errors before submitting.", tone: "error" });
        return false;
      }
      return true;
    } catch (error) {
      setValidationWarnings([]);
      setValidationErrors([]);
      setNotice({ message: friendlyApiError(error), tone: "error" });
      return false;
    } finally {
      setValidating(false);
      window.setTimeout(() => setOperationProgress(undefined), 900);
    }
  }, [activeChangelist, api, browserUploads, demoMode, selectedWorkspace, session]);

  useEffect(() => {
    if (activeView !== "history" || !selectedFile) return;
    if (demoMode) {
      setHistoryEntries(demoHistory(selectedFile, session?.username ?? "demo.artist"));
      setHistoryError(undefined);
      return;
    }
    if (!session || !api || !selectedWorkspace) return;

    let cancelled = false;
    setHistoryLoading(true);
    setHistoryError(undefined);
    void api.fileHistory(session.token, selectedWorkspace.id, selectedFile.path)
      .then((entries) => {
        if (!cancelled) setHistoryEntries([...entries].reverse());
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setHistoryEntries([]);
          setHistoryError(friendlyApiError(error));
        }
      })
      .finally(() => {
        if (!cancelled) setHistoryLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [activeView, api, demoMode, selectedFile, selectedWorkspace, session]);

  const selectFile = useCallback((file: AssetFile) => {
    setSelectedFileId(file.id);
  }, []);

  const openFile = useCallback((file: AssetFile) => {
    selectFile(file);
    setActiveView("workspace");
  }, [selectFile]);

  const openSubmit = useCallback((changelist = activeChangelist) => {
    const selected = demoMode ? changelist : liveChangelist;
    if (!selected) {
      setNotice({ message: "There are no pending files to submit.", tone: "info" });
      return;
    }
    setActiveChangelist(selected);
    setSubmitOpen(true);
  }, [activeChangelist, demoMode, liveChangelist]);

  const performFileAction = useCallback(async (
    action: "lock" | "unlock" | "add" | "delete" | "revert",
    file: AssetFile,
  ) => {
    if (!session || !api || !selectedWorkspace) return;
    if (demoMode) {
      setNotice({ message: `${actionLabel(action)} previewed in Demo Mode.`, tone: "info" });
      return;
    }
    if (!isNativeDesktop() && action === "revert" && browserUploads.some((upload) => upload.path === file.path)) {
      setActionBusy(true);
      try {
        if (browserChangelistId) {
          await api.revertFile(session.token, selectedWorkspace.id, file.path, browserChangelistId);
        }
        const remaining = browserUploads.filter((upload) => upload.path !== file.path);
        setBrowserUploads(remaining);
        setPendingFiles(remaining.map((upload) => ({
          path: upload.path,
          local_path: upload.file.name,
          action: upload.action,
        })));
        setNotice({ message: `Removed ${file.name} from the browser upload.`, tone: "success" });
      } catch (error) {
        setNotice({ message: friendlyApiError(error), tone: "error" });
      } finally {
        setActionBusy(false);
      }
      return;
    }
    if (!isNativeDesktop() && action !== "unlock") {
      setNotice({ message: `${actionLabel(action)} changes local project state and requires the desktop app.`, tone: "info" });
      return;
    }

    setActionBusy(true);
    try {
      if (isNativeDesktop()) {
        const command = action === "lock" ? "checkout" : action;
        await runWorkspaceIntegration(session, selectedWorkspace, command, {
          paths: [file.path],
          reason: action === "lock" ? "Checked out from OpenAsset Depot desktop" : undefined,
          onProgress: setOperationProgress,
        });
        await Promise.all([refreshLocks(), refreshPending()]);
      } else if (action === "lock") {
        await api.lockFile(session.token, selectedWorkspace.id, file.path, "Checked out from desktop");
        await refreshLocks();
      } else if (action === "unlock") {
        await api.unlockFile(session.token, selectedWorkspace.id, file.path);
        await refreshLocks();
      } else if (action === "add") {
        await api.addFile(session.token, selectedWorkspace.id, file.path);
      } else if (action === "delete") {
        await api.deleteFile(session.token, selectedWorkspace.id, file.path);
      } else {
        await api.revertFile(session.token, selectedWorkspace.id, file.path);
      }
      setNotice({ message: `${actionLabel(action)} completed for ${file.name}.`, tone: "success" });
    } catch (error) {
      setNotice({ message: friendlyApiError(error), tone: "error" });
    } finally {
      setActionBusy(false);
      window.setTimeout(() => setOperationProgress(undefined), 900);
    }
  }, [api, browserChangelistId, browserUploads, demoMode, refreshLocks, refreshPending, selectedWorkspace, session]);

  const addWorkspaceFiles = useCallback(async (browserFiles?: File[]) => {
    if (!session || !selectedWorkspace || demoMode) return;
    if (!isNativeDesktop()) {
      if (!browserFiles || browserFiles.length === 0) return;
      const byPath = new Map(browserUploads.map((upload) => [upload.path, upload]));
      for (const file of browserFiles) {
        const path = browserUploadPath(file);
        const tracked = workspaceFiles.some((entry) => entry.path === path && !entry.deleted);
        byPath.set(path, { path, file, action: tracked ? "edit" : "add" });
      }
      const uploads = Array.from(byPath.values()).sort((left, right) => left.path.localeCompare(right.path));
      setBrowserUploads(uploads);
      setPendingFiles(uploads.map((upload) => ({
        path: upload.path,
        local_path: upload.file.name,
        action: upload.action,
      })));
      setActiveChangelist({
        id: browserChangelistId ?? "browser-upload",
        title: "Browser Upload",
        description: "Uploaded from the web workspace",
        files: uploads.map((upload) => ({
          ...assetFromPendingPath(upload.path, upload.action),
          size: formatBytes(upload.file.size),
        })),
        warnings: [],
        ready: true,
        source: "backend",
      });
      setSubmitOpen(true);
      setNotice({
        message: `Selected ${uploads.length} ${uploads.length === 1 ? "file" : "files"}. Review and submit the upload.`,
        tone: "success",
      });
      return;
    }
    const selected = await chooseWorkspaceFiles(selectedWorkspace.local_path);
    if (selected.length === 0) return;
    setActionBusy(true);
    try {
      for (const path of selected) {
        await runWorkspaceIntegration(session, selectedWorkspace, "add", {
          paths: [path],
          onProgress: setOperationProgress,
        });
      }
      await refreshPending();
      setNotice({
        message: `Added ${selected.length} ${selected.length === 1 ? "file" : "files"} to Workspace Changes.`,
        tone: "success",
      });
    } catch (error) {
      setNotice({ message: friendlyApiError(error), tone: "error" });
    } finally {
      setActionBusy(false);
      window.setTimeout(() => setOperationProgress(undefined), 900);
    }
  }, [browserChangelistId, browserUploads, demoMode, refreshPending, selectedWorkspace, session, workspaceFiles]);

  const submitChanges = useCallback(async (description: string) => {
    if (demoMode) {
      setSubmitOpen(false);
      setNotice({ message: "Submission preview completed in Demo Mode.", tone: "info" });
      return;
    }
    if (!session || !selectedWorkspace || !api) {
      setNotice({ message: "Connect to the server before submitting changes.", tone: "error" });
      return;
    }
    if (!description) {
      setNotice({ message: "Add a submit description before submitting.", tone: "error" });
      return;
    }
    setActionBusy(true);
    try {
      if (isNativeDesktop()) {
        await runWorkspaceIntegration(session, selectedWorkspace, "submit", {
          description,
          onProgress: setOperationProgress,
        });
      } else {
        if (browserUploads.length === 0) throw new Error("Choose one or more files to upload first.");
        if (!await validateSubmit()) return;
        let changelistId = browserChangelistId;
        if (!changelistId) {
          const changelist = await api.createChangelist(session.token, selectedWorkspace.id, description);
          changelistId = changelist.id;
          setBrowserChangelistId(changelistId);
        }
        for (const upload of browserUploads) {
          if (upload.action === "edit") {
            await api.editFile(session.token, selectedWorkspace.id, upload.path, changelistId);
          } else {
            await api.addFile(session.token, selectedWorkspace.id, upload.path, changelistId);
          }
        }
        await api.submitChangelist(
          session.token,
          selectedWorkspace.id,
          changelistId,
          browserUploads,
        );
        setBrowserUploads([]);
        setBrowserChangelistId(undefined);
        setPendingFiles([]);
      }
      setSubmitOpen(false);
      await Promise.all([
        refreshPending(),
        refreshLocks(),
        refreshSyncPlan(false),
        refreshWorkspaceFiles(),
      ]);
      setNotice({
        message: isNativeDesktop()
          ? "Changes submitted successfully."
          : "Browser upload submitted. Desktop and DCC workspaces can sync it now.",
        tone: "success",
      });
    } catch (error) {
      setNotice({ message: friendlyApiError(error), tone: "error" });
    } finally {
      setActionBusy(false);
      window.setTimeout(() => setOperationProgress(undefined), 900);
    }
  }, [
    demoMode,
    api,
    browserChangelistId,
    browserUploads,
    refreshLocks,
    refreshPending,
    refreshSyncPlan,
    refreshWorkspaceFiles,
    selectedWorkspace,
    session,
    validateSubmit,
  ]);

  const unlockFromLockCenter = useCallback((lock: LockInfo) => {
    const file = files.find((candidate) => candidate.path === lock.depot_path) ?? {
      id: lock.depot_path,
      name: lock.depot_path.split("/").at(-1) ?? lock.depot_path,
      path: lock.depot_path,
      kind: "Asset",
      size: "",
      revision: 0,
      statuses: ["Checked Out"],
      previewTone: "document",
      dependencies: [],
      source: demoMode ? "mock" : "backend",
    } satisfies AssetFile;
    void performFileAction("unlock", file);
  }, [demoMode, files, performFileAction]);

  const commands: CommandAction[] = useMemo(
    () => [
      { id: "sync", label: "Sync Latest", group: "Workspace", run: () => void syncLatest() },
      { id: "submit", label: "Submit Changes", group: "Changes", run: () => openSubmit() },
      {
        id: "lock",
        label: "Lock / Check Out Selected File",
        group: "Workspace",
        run: () => selectedFile && void performFileAction("lock", selectedFile),
      },
      {
        id: "unlock",
        label: "Unlock Selected File",
        group: "Workspace",
        run: () => selectedFile && void performFileAction("unlock", selectedFile),
      },
      { id: "my-locks", label: "Show Files Locked by Me", group: "Locks", run: () => setActiveView("locks") },
      { id: "project", label: "Open Project", group: "Workspace", run: () => setActiveView("workspace") },
      { id: "switch", label: "Switch Workspace", group: "Workspace", run: () => setSelectedWorkspace(undefined) },
      { id: "history", label: "Search History", group: "History", run: () => setActiveView("history") },
      { id: "settings", label: "Open Settings", group: "Settings", run: () => setActiveView("settings") },
      {
        id: "validate",
        label: "Validate Workspace",
        group: "Changes",
        run: () => {
          if (!demoMode) {
            setActiveView("changes");
            return;
          }
          setSubmitOpen(true);
          void validateSubmit();
        },
      },
    ],
    [demoMode, openSubmit, performFileAction, selectedFile, syncLatest, validateSubmit],
  );

  if (!session) {
    return <Login onLogin={setSessionAndStore} />;
  }

  if (!selectedWorkspace) {
    return (
      <WorkspaceSelector
        username={session.username}
        workspaces={workspaces}
        depots={depots}
        streams={streams}
        loading={loadingWorkspaces}
        error={workspaceError}
        isAdmin={session.isAdmin}
        onRefresh={refreshBackendData}
        onCreateDepot={createDepot}
        onCreateStream={createStream}
        onCreate={createWorkspace}
        onSelect={setSelectedWorkspace}
      />
    );
  }

  const currentSession = session;

  return (
    <div className="app-shell">
      <Sidebar
        activeView={activeView}
        connectionState={connectionMode === "connected" ? "connected" : connectionMode === "connecting" || connectionMode === "reconnecting" ? "reconnecting" : "offline"}
        isAdmin={currentSession.isAdmin}
        onNavigate={setActiveView}
      />
      <div className="app-main">
        <TopBar
          session={currentSession}
          workspace={selectedWorkspace!}
          theme={theme}
          onOpenPalette={() => setPaletteOpen(true)}
          onToggleTheme={() => setTheme((value) => (value === "light" ? "dark" : "light"))}
          onSwitchWorkspace={() => setSelectedWorkspace(undefined)}
          onLogout={logout}
        />
        <div className="app-content">
          {demoMode && (
            <ConnectionStatusBar
              retrying={loadingWorkspaces || actionBusy}
              onRetry={() => void retryConnection()}
              onOpenSettings={() => setActiveView("settings")}
            />
          )}
          {notice && <ActionNotice {...notice} onClose={() => setNotice(undefined)} />}
          <OperationProgressBar progress={operationProgress} />
          {renderView(activeView)}
        </div>
      </div>
      <CommandPalette open={paletteOpen} actions={commands} onClose={() => setPaletteOpen(false)} />
      <SubmitDialog
        open={submitOpen}
        changelist={activeChangelist}
        validating={validating}
        validationWarnings={validationWarnings}
        validationErrors={validationErrors}
        onValidate={validateSubmit}
        onSubmit={(description) => void submitChanges(description)}
        onClose={() => setSubmitOpen(false)}
      />
      <ReviewViewer
        file={reviewFile}
        open={Boolean(reviewFile)}
        onClose={() => setReviewFile(undefined)}
        loadMedia={loadReviewMedia}
        loadComments={loadReviewComments}
        createComment={createReviewComment}
        resolveComment={resolveReviewComment}
      />
    </div>
  );

  function renderView(view: ViewKey) {
    if (view === "workspace") {
      return (
        <Workspace
          files={files}
          selectedFile={selectedFile}
          onSelectFile={selectFile}
          onSync={() => void syncLatest()}
          onChooseFiles={(selected) => void addWorkspaceFiles(selected)}
          browserMode={!isNativeDesktop()}
          onLock={(file) => void performFileAction("lock", file)}
          onUnlock={(file) => void performFileAction("unlock", file)}
          onRevert={(file) => void performFileAction("revert", file)}
          onDelete={(file) => void performFileAction("delete", file)}
          onSubmit={() => openSubmit()}
          onReview={setReviewFile}
          onHistory={() => setActiveView("history")}
          hasMoreFiles={workspaceFilesHasMore}
          onLoadMore={() => void loadMoreWorkspaceFiles()}
          busy={actionBusy}
          loadPreview={loadAssetPreview}
        />
      );
    }
    if (view === "changes") {
      return (
        <Changes
          changelists={changelists}
          onSubmit={openSubmit}
          onBrowseWorkspace={() => setActiveView("workspace")}
        />
      );
    }
    if (view === "locks") {
      return (
        <Locks
          locks={locks}
          currentUser={currentSession.username}
          workspace={selectedWorkspace!}
          loading={loadingLocks}
          onRefresh={() => void refreshLocks()}
          onUnlock={unlockFromLockCenter}
        />
      );
    }
    if (view === "admin") {
      if (!currentSession.isAdmin) {
        return (
          <Home
            workspace={selectedWorkspace}
            files={files}
            changelists={changelists}
            locks={locks}
            currentUser={currentSession.username}
            onNavigate={setActiveView}
            onSelectFile={openFile}
            onSync={() => void syncLatest()}
            onSubmit={() => openSubmit()}
            busy={actionBusy}
            loadPreview={loadAssetPreview}
          />
        );
      }
      return (
        <Admin
          depots={depots}
          streams={streams}
          filetypes={filetypes}
          onCreateDepot={async (name, description) => { await createDepot(name, description); }}
          onCreateStream={async (name, depotId) => { await createStream(name, depotId); }}
          onRefresh={refreshBackendData}
        />
      );
    }
    if (view === "settings") {
      return (
        <Settings
          session={currentSession}
          filetypes={filetypes}
          workspace={selectedWorkspace!}
          onRefreshFiletypes={refreshBackendData}
          onDeleteWorkspace={deleteWorkspace}
          deletingWorkspace={actionBusy}
          onLogout={() => {
            logout();
          }}
          onChangePassword={async (currentPassword, newPassword) => {
            if (!api) throw new Error("Server connection is unavailable.");
            try {
              await api.changePassword(currentSession.token, currentPassword, newPassword);
            } catch (error) {
              throw new Error(friendlyApiError(error));
            }
          }}
        />
      );
    }
    if (view === "history") {
      return (
        <History
          file={selectedFile}
          entries={historyEntries}
          loading={historyLoading}
          error={historyError}
          onBrowseWorkspace={() => setActiveView("workspace")}
        />
      );
    }
    return (
      <Home
        workspace={selectedWorkspace}
        files={files}
        changelists={changelists}
        locks={locks}
        currentUser={currentSession.username}
        onNavigate={setActiveView}
        onSelectFile={openFile}
        onSync={() => void syncLatest()}
        onSubmit={() => openSubmit()}
        busy={actionBusy}
        loadPreview={loadAssetPreview}
      />
    );
  }

  function setSessionAndStore(nextSession: UserSession) {
    sessionStorage.setItem("oad.session", JSON.stringify(nextSession));
    setConnectionMode(nextSession.token === "mock-preview-token" ? "demo" : "connecting");
    setSelectedWorkspace(undefined);
    setSession(nextSession);
  }
}

function actionLabel(action: "lock" | "unlock" | "add" | "delete" | "revert"): string {
  if (action === "lock") return "Lock / Check Out";
  if (action === "unlock") return "Unlock";
  if (action === "add") return "Add File";
  if (action === "delete") return "Mark for Delete";
  return "Revert Intent";
}

function escapeSvg(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}

function browserUploadPath(file: File): string {
  const relativePath = file.webkitRelativePath || file.name;
  return relativePath
    .replaceAll("\\", "/")
    .replace(/^\/+/, "")
    .split("/")
    .filter((part) => part && part !== ".")
    .join("/");
}

function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  if (size < 1024 * 1024 * 1024) return `${(size / (1024 * 1024)).toFixed(1)} MB`;
  return `${(size / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function demoHistory(file: AssetFile, username: string): FileHistoryEntry[] {
  return [file.revision, file.revision - 1, file.revision - 2]
    .filter((revision) => revision > 0)
    .map((revision, index) => ({
      revision_number: revision,
      blob_hash: `demo-${file.id}-${revision}`,
      size_bytes: 0,
      action: index === 0 ? "edit" : "submit",
      submitted_by: username,
      submitted_at: new Date(Date.now() - index * 86_400_000).toISOString(),
    }));
}

function readStoredSession(): UserSession | undefined {
  const value = sessionStorage.getItem("oad.session");
  if (!value) return undefined;
  try {
    const session = JSON.parse(value) as Omit<UserSession, "isAdmin"> & { isAdmin?: boolean };
    return { ...session, isAdmin: session.isAdmin === true };
  } catch {
    return undefined;
  }
}

function readStoredTheme(): Theme {
  return localStorage.getItem("oad.theme") === "dark" ? "dark" : "light";
}
