import type {
  AdapterDefinition,
  AdapterFileInput,
  AdapterValidationResponse,
  Collaborator,
  ChangelistResponse,
  CreateReviewRequestInput,
  CreateWorkspaceInput,
  DeleteWorkspaceResponse,
  Depot,
  DependencyImpact,
  DependencyScanResult,
  DiscardShelfResponse,
  ExtractedMetadata,
  FileHistoryEntry,
  FileOperationResponse,
  FileTypeRule,
  LockInfo,
  LockPageResponse,
  PreviewGeneration,
  ProjectDetection,
  CreateReviewCommentInput,
  ReviewComment,
  ReviewRequest,
  Shelf,
  ShelfSummary,
  Stream,
  SignupStatus,
  SsoStart,
  SsoStatus,
  SubmitResponse,
  SyncPlanEntry,
  UserSession,
  UserAccount,
  ValidationResponse,
  Workspace,
} from "../types/domain";
import { createUuid } from "../utils/uuid";

export class ApiError extends Error {
  readonly status: number;
  readonly code?: string;

  constructor(message: string, status: number, code?: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

interface LoginResponse {
  token: string;
  user: {
    username: string;
    is_admin: boolean;
  };
}

interface HealthResponse {
  status: string;
}

export interface BrowserUpload {
  path: string;
  file: File;
  action: "add" | "edit";
}

export class OpenAssetApiClient {
  private readonly baseUrl: string;
  private readonly fetcher: typeof fetch;

  constructor(baseUrl: string, fetcher?: typeof fetch) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.fetcher = fetcher ?? ((input, init) => fetch(input, init));
  }

  async login(username: string, password: string): Promise<UserSession> {
    const response = await this.request<LoginResponse>("/api/auth/login", {
      method: "POST",
      body: JSON.stringify({ username, password }),
    });
    return {
      token: response.token,
      username: response.user.username,
      serverUrl: this.baseUrl,
      isAdmin: response.user.is_admin,
    };
  }

  signupStatus(): Promise<SignupStatus> {
    return this.request<SignupStatus>("/api/auth/signup");
  }

  /** Reports whether this server has an identity provider configured. */
  ssoStatus(): Promise<SsoStatus> {
    return this.request<SsoStatus>("/api/auth/sso");
  }

  /** Begins an SSO login and returns the URL to open in a browser. */
  startSso(): Promise<SsoStart> {
    return this.request<SsoStart>("/api/auth/sso", { method: "POST" });
  }

  /** Exchanges the authorization code from the redirect for a session. */
  async completeSso(code: string, state: string): Promise<UserSession> {
    const response = await this.request<LoginResponse>("/api/auth/sso/callback", {
      method: "POST",
      body: JSON.stringify({ code, state }),
    });
    return {
      token: response.token,
      username: response.user.username,
      serverUrl: this.baseUrl,
      isAdmin: response.user.is_admin,
    };
  }

  signup(username: string, password: string, displayName?: string): Promise<UserAccount> {
    return this.request<UserAccount>("/api/auth/signup", {
      method: "POST",
      body: JSON.stringify({ username, password, display_name: displayName }),
    });
  }

  changePassword(token: string, currentPassword: string, newPassword: string): Promise<{ changed: boolean }> {
    return this.request<{ changed: boolean }>("/api/auth/change-password", {
      method: "POST",
      token,
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    });
  }

  ready(): Promise<HealthResponse> {
    return this.request<HealthResponse>("/ready");
  }

  listDepots(token: string): Promise<Depot[]> {
    return this.request<Depot[]>("/api/depots", { token });
  }

  createDepot(token: string, name: string, description?: string): Promise<Depot> {
    return this.request<Depot>("/api/depots", {
      method: "POST",
      token,
      headers: { "idempotency-key": createUuid() },
      body: JSON.stringify({ name, description }),
    });
  }

  createStream(token: string, name: string, depot: string): Promise<Stream> {
    return this.request<Stream>("/api/streams", {
      method: "POST",
      token,
      headers: { "idempotency-key": createUuid() },
      body: JSON.stringify({ name, depot }),
    });
  }

  listWorkspaces(token: string): Promise<Workspace[]> {
    return this.request<Workspace[]>("/api/workspaces", { token });
  }

  createWorkspace(token: string, input: CreateWorkspaceInput): Promise<Workspace> {
    return this.request<Workspace>("/api/workspaces", {
      method: "POST",
      token,
      headers: { "idempotency-key": createUuid() },
      body: JSON.stringify(input),
    });
  }

  createChangelist(token: string, workspaceId: string, description: string): Promise<ChangelistResponse> {
    return this.request<ChangelistResponse>("/api/changelists", {
      method: "POST",
      token,
      headers: { "idempotency-key": createUuid() },
      body: JSON.stringify({ workspace_id: workspaceId, description }),
    });
  }

  submitChangelist(
    token: string,
    workspaceId: string,
    changelistId: string,
    uploads: BrowserUpload[],
  ): Promise<SubmitResponse> {
    const form = new FormData();
    form.set("workspace_id", workspaceId);
    for (const upload of uploads) {
      form.append("file", upload.file, upload.path);
    }
    return this.request<SubmitResponse>(`/api/changelists/${changelistId}/submit`, {
      method: "POST",
      token,
      body: form,
    });
  }

  /**
   * Parks a changelist's file content on the server without submitting it.
   *
   * Uses the same multipart shape as submit, so a caller that can build a submit
   * can shelve. Unlike submit this creates no revision and needs no lock.
   */
  shelveChangelist(
    token: string,
    changelistId: string,
    uploads: BrowserUpload[],
  ): Promise<Shelf> {
    const form = new FormData();
    for (const upload of uploads) {
      form.append("file", upload.file, upload.path);
    }
    return this.request<Shelf>(`/api/changelists/${changelistId}/shelve`, {
      method: "POST",
      token,
      body: form,
    });
  }

  getShelf(token: string, changelistId: string): Promise<Shelf> {
    return this.request<Shelf>(`/api/changelists/${changelistId}/shelve`, { token });
  }

  listShelves(token: string, workspaceId: string): Promise<ShelfSummary[]> {
    const query = new URLSearchParams({ workspace_id: workspaceId });
    return this.request<ShelfSummary[]>(`/api/shelves?${query}`, { token });
  }

  unshelveChangelist(token: string, changelistId: string): Promise<Shelf> {
    return this.request<Shelf>(`/api/changelists/${changelistId}/unshelve`, {
      method: "POST",
      token,
      headers: { "idempotency-key": createUuid() },
      body: JSON.stringify({}),
    });
  }

  discardShelf(token: string, changelistId: string): Promise<DiscardShelfResponse> {
    return this.request<DiscardShelfResponse>(`/api/changelists/${changelistId}/shelve`, {
      method: "DELETE",
      token,
      headers: { "idempotency-key": createUuid() },
    });
  }

  /** Downloads one shelved file's bytes, or `undefined` if the shelf lost it. */
  async downloadShelfContent(
    token: string,
    changelistId: string,
    path: string,
  ): Promise<Blob | undefined> {
    const query = new URLSearchParams({ changelist_id: changelistId, path });
    const response = await this.fetcher(`${this.baseUrl}/api/shelves/content?${query}`, {
      headers: { authorization: `Bearer ${token}` },
    });
    if (response.status === 404) return undefined;
    if (!response.ok) throw new ApiError(`Request failed with ${response.status}`, response.status);
    return await response.blob();
  }

  /**
   * Reports what depends on a file and what it depends on.
   *
   * The reverse direction is the one that changes an artist's decision, so this
   * is fetched when the inspector opens rather than behind a disclosure.
   */
  dependencyImpact(token: string, workspaceId: string, path: string): Promise<DependencyImpact> {
    const query = new URLSearchParams({ workspace_id: workspaceId, path });
    return this.request<DependencyImpact>(`/api/dependencies/impact?${query}`, { token });
  }

  listReviewRequests(
    token: string,
    workspaceId: string,
    options: { state?: ReviewRequest["state"]; awaitingMe?: boolean } = {},
  ): Promise<ReviewRequest[]> {
    const query = new URLSearchParams({ workspace_id: workspaceId });
    if (options.state) query.set("state", options.state);
    if (options.awaitingMe) query.set("awaiting_me", "true");
    return this.request<ReviewRequest[]>(`/api/reviews/requests?${query}`, { token });
  }

  createReviewRequest(token: string, input: CreateReviewRequestInput): Promise<ReviewRequest> {
    return this.request<ReviewRequest>("/api/reviews/requests", {
      method: "POST",
      token,
      headers: { "idempotency-key": createUuid() },
      body: JSON.stringify({
        workspace_id: input.workspaceId,
        path: input.path,
        revision_number: input.revisionNumber,
        title: input.title,
        description: input.description ?? "",
        reviewers: input.reviewers,
      }),
    });
  }

  decideReviewRequest(
    token: string,
    workspaceId: string,
    requestId: string,
    decision: "approved" | "changes_requested",
    note?: string,
  ): Promise<ReviewRequest> {
    return this.request<ReviewRequest>(`/api/reviews/requests/${requestId}/decision`, {
      method: "POST",
      token,
      headers: { "idempotency-key": createUuid() },
      body: JSON.stringify({ workspace_id: workspaceId, decision, note }),
    });
  }

  closeReviewRequest(
    token: string,
    workspaceId: string,
    requestId: string,
  ): Promise<ReviewRequest> {
    return this.request<ReviewRequest>(`/api/reviews/requests/${requestId}/close`, {
      method: "POST",
      token,
      headers: { "idempotency-key": createUuid() },
      body: JSON.stringify({ workspace_id: workspaceId }),
    });
  }

  /** People who can open this workspace's depot, for assigning reviewers. */
  listCollaborators(token: string, workspaceId: string): Promise<Collaborator[]> {
    return this.request<Collaborator[]>(`/api/workspaces/${workspaceId}/collaborators`, { token });
  }

  deleteWorkspace(token: string, workspaceId: string): Promise<DeleteWorkspaceResponse> {
    return this.request<DeleteWorkspaceResponse>(`/api/workspaces/${workspaceId}`, {
      method: "DELETE",
      token,
    });
  }

  listStreams(token: string): Promise<Stream[]> {
    return this.request<Stream[]>("/api/streams", { token });
  }

  async listLocks(token: string): Promise<LockInfo[]> {
    const locks: LockInfo[] = [];
    let beforeCreatedAt: string | undefined;
    let beforeId: string | undefined;
    for (;;) {
      const query = new URLSearchParams({ limit: "1000" });
      if (beforeCreatedAt && beforeId) {
        query.set("before_created_at", beforeCreatedAt);
        query.set("before_id", beforeId);
      }
      const page = await this.request<LockPageResponse>(`/api/locks/page?${query}`, { token });
      locks.push(...page.items);
      const nextCreatedAt = page.next_before_created_at ?? undefined;
      const nextId = page.next_before_id ?? undefined;
      if (!nextCreatedAt || !nextId) return locks;
      if (nextCreatedAt === beforeCreatedAt && nextId === beforeId) {
        throw new ApiError("Server returned a repeated lock-page cursor", 502, "invalid_page_cursor");
      }
      beforeCreatedAt = nextCreatedAt;
      beforeId = nextId;
    }
  }

  listFiletypes(token: string): Promise<FileTypeRule[]> {
    return this.request<FileTypeRule[]>("/api/filetypes", { token });
  }

  planSync(token: string, workspaceId: string, pathPrefix?: string): Promise<SyncPlanEntry[]> {
    return this.request<SyncPlanEntry[]>("/api/sync/plan", {
      method: "POST",
      token,
      body: JSON.stringify({
        workspace_id: workspaceId,
        path_prefix: pathPrefix,
      }),
    });
  }

  listWorkspaceFiles(
    token: string,
    workspaceId: string,
    afterPath?: string,
    limit = 1_000,
  ): Promise<SyncPlanEntry[]> {
    return this.request<SyncPlanEntry[]>("/api/sync/plan", {
      method: "POST",
      token,
      body: JSON.stringify({
        workspace_id: workspaceId,
        after_path: afterPath,
        limit,
        include_current: true,
      }),
    });
  }

  async downloadPreview(
    token: string,
    workspaceId: string,
    path: string,
    revisionNumber: number,
  ): Promise<Blob | undefined> {
    const query = new URLSearchParams({
      workspace_id: workspaceId,
      path,
      revision_number: String(revisionNumber),
    });
    const response = await this.fetcher(`${this.baseUrl}/api/files/preview?${query}`, {
      headers: { authorization: `Bearer ${token}` },
    });
    if (response.status === 404) return undefined;
    if (!response.ok) {
      throw new ApiError(`Preview request failed with ${response.status}`, response.status);
    }
    return response.blob();
  }

  async downloadReviewMedia(
    token: string,
    workspaceId: string,
    path: string,
    revisionNumber: number,
  ): Promise<Blob | undefined> {
    const query = new URLSearchParams({
      workspace_id: workspaceId,
      path,
      revision_number: String(revisionNumber),
    });
    const response = await this.fetcher(`${this.baseUrl}/api/reviews/media?${query}`, {
      headers: { authorization: `Bearer ${token}` },
    });
    if (response.status === 404) return undefined;
    if (!response.ok) {
      throw new ApiError(`Review media request failed with ${response.status}`, response.status);
    }
    return response.blob();
  }

  async downloadReviewProxy(
    token: string,
    workspaceId: string,
    path: string,
    revisionNumber: number,
  ): Promise<Blob | undefined> {
    const query = new URLSearchParams({
      workspace_id: workspaceId,
      path,
      revision_number: String(revisionNumber),
    });
    const response = await this.fetcher(`${this.baseUrl}/api/reviews/proxy?${query}`, {
      headers: { authorization: `Bearer ${token}` },
    });
    if (response.status === 404) return undefined;
    if (!response.ok) {
      throw new ApiError(`Review proxy request failed with ${response.status}`, response.status);
    }
    return response.blob();
  }

  listReviewComments(
    token: string,
    workspaceId: string,
    path: string,
    revisionNumber: number,
  ): Promise<ReviewComment[]> {
    const query = new URLSearchParams({
      workspace_id: workspaceId,
      path,
      revision_number: String(revisionNumber),
    });
    return this.request<ReviewComment[]>(`/api/reviews/comments?${query}`, { token });
  }

  createReviewComment(token: string, input: CreateReviewCommentInput): Promise<ReviewComment> {
    return this.request<ReviewComment>("/api/reviews/comments", {
      method: "POST",
      token,
      body: JSON.stringify(input),
    });
  }

  resolveReviewComment(
    token: string,
    workspaceId: string,
    commentId: string,
    resolved = true,
  ): Promise<ReviewComment> {
    return this.request<ReviewComment>(`/api/reviews/comments/${commentId}/resolve`, {
      method: "POST",
      token,
      body: JSON.stringify({ workspace_id: workspaceId, resolved }),
    });
  }

  lockFile(token: string, workspaceId: string, path: string, reason?: string): Promise<LockInfo> {
    return this.request<LockInfo>("/api/files/lock", {
      method: "POST",
      token,
      body: JSON.stringify({ workspace_id: workspaceId, path, reason }),
    });
  }

  unlockFile(token: string, workspaceId: string, path: string): Promise<LockInfo> {
    return this.request<LockInfo>("/api/files/unlock", {
      method: "POST",
      token,
      body: JSON.stringify({ workspace_id: workspaceId, path }),
    });
  }

  addFile(
    token: string,
    workspaceId: string,
    path: string,
    changelistId?: string,
  ): Promise<FileOperationResponse> {
    return this.fileOperation(token, "/api/files/add", workspaceId, path, changelistId);
  }

  editFile(
    token: string,
    workspaceId: string,
    path: string,
    changelistId?: string,
  ): Promise<FileOperationResponse> {
    return this.fileOperation(token, "/api/files/edit", workspaceId, path, changelistId);
  }

  deleteFile(
    token: string,
    workspaceId: string,
    path: string,
    changelistId?: string,
  ): Promise<FileOperationResponse> {
    return this.fileOperation(token, "/api/files/delete", workspaceId, path, changelistId);
  }

  revertFile(
    token: string,
    workspaceId: string,
    path: string,
    changelistId?: string,
  ): Promise<FileOperationResponse> {
    return this.fileOperation(token, "/api/files/revert", workspaceId, path, changelistId);
  }

  fileHistory(
    token: string,
    workspaceId: string,
    path: string,
    limit = 100,
  ): Promise<FileHistoryEntry[]> {
    const query = new URLSearchParams({
      workspace_id: workspaceId,
      path,
      limit: String(limit),
    });
    return this.request<FileHistoryEntry[]>(`/api/files/history?${query.toString()}`, { token });
  }

  listAdapters(token: string): Promise<AdapterDefinition[]> {
    return this.request<AdapterDefinition[]>("/api/adapters", { token });
  }

  detectProject(
    token: string,
    rootPath: string,
    relativePaths: string[],
  ): Promise<ProjectDetection[]> {
    return this.request<ProjectDetection[]>("/api/adapters/detect", {
      method: "POST",
      token,
      body: JSON.stringify({ root_path: rootPath, relative_paths: relativePaths }),
    });
  }

  scanAdapterDependencies(
    token: string,
    file: AdapterFileInput,
    adapterName?: string,
    workspaceId?: string,
  ): Promise<DependencyScanResult> {
    return this.request<DependencyScanResult>("/api/adapters/scan", {
      method: "POST",
      token,
      body: JSON.stringify({ adapter_name: adapterName, workspace_id: workspaceId, file }),
    });
  }

  generateAdapterPreview(
    token: string,
    file: AdapterFileInput,
    adapterName?: string,
  ): Promise<PreviewGeneration> {
    return this.request<PreviewGeneration>("/api/adapters/preview", {
      method: "POST",
      token,
      body: JSON.stringify({ adapter_name: adapterName, file }),
    });
  }

  extractAdapterMetadata(
    token: string,
    file: AdapterFileInput,
    adapterName?: string,
  ): Promise<ExtractedMetadata> {
    return this.request<ExtractedMetadata>("/api/adapters/metadata", {
      method: "POST",
      token,
      body: JSON.stringify({ adapter_name: adapterName, file }),
    });
  }

  validateAdapterChangelist(
    token: string,
    files: AdapterFileInput[],
    adapterName?: string,
  ): Promise<AdapterValidationResponse> {
    return this.request<AdapterValidationResponse>("/api/adapters/validate", {
      method: "POST",
      token,
      body: JSON.stringify({ adapter_name: adapterName, files }),
    });
  }

  validateWorkspace(
    token: string,
    workspaceId: string,
    paths: Array<{ path: string; size_bytes?: number }>,
  ): Promise<ValidationResponse> {
    return this.request<ValidationResponse>("/api/validate", {
      method: "POST",
      token,
      body: JSON.stringify({ workspace_id: workspaceId, paths }),
    });
  }

  private fileOperation(
    token: string,
    endpoint: string,
    workspaceId: string,
    path: string,
    changelistId?: string,
  ): Promise<FileOperationResponse> {
    return this.request<FileOperationResponse>(endpoint, {
      method: "POST",
      token,
      body: JSON.stringify({
        workspace_id: workspaceId,
        changelist_id: changelistId,
        path,
      }),
    });
  }

  private async request<T>(
    path: string,
    options: RequestInit & { token?: string } = {},
  ): Promise<T> {
    const headers = new Headers(options.headers);
    if (!(options.body instanceof FormData)) {
      headers.set("content-type", "application/json");
    }
    if (options.token) {
      headers.set("authorization", `Bearer ${options.token}`);
    }

    const response = await this.fetcher(`${this.baseUrl}${path}`, {
      ...options,
      headers,
    });

    if (!response.ok) {
      let message = `Request failed with ${response.status}`;
      let code: string | undefined;
      try {
        const body = (await response.json()) as { message?: string; code?: string };
        message = body.message ?? message;
        code = body.code;
      } catch {
        const text = await response.text();
        if (text) message = text;
      }
      throw new ApiError(message, response.status, code);
    }

    return (await response.json()) as T;
  }
}

export function friendlyApiError(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.status === 401 && error.message.toLowerCase().includes("current password")) {
      return error.message;
    }
    if (error.status === 401) return "Sign in again to continue.";
    if (error.status === 409) return humanConflictMessage(error.message);
    return error.message;
  }
  if (error instanceof TypeError) {
    return "The desktop app cannot reach the OpenAsset Depot server.";
  }
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

function humanConflictMessage(message: string): string {
  const normalized = message.toLowerCase();
  if (normalized.includes("already locked") || normalized.includes("active lock")) {
    return "This asset is checked out by another artist. Ask them to submit or unlock it, then try again.";
  }
  if (normalized.includes("pending work") || normalized.includes("locally modified")) {
    return "Sync paused to protect local work. Submit or revert those changes before syncing again.";
  }
  if (normalized.includes("untracked file")) {
    return "Sync found a local file in the way. Add it to the depot or move it aside, then try again.";
  }
  return `${message}. Nothing was changed; refresh the workspace and try again.`;
}

export function isConnectionError(error: unknown): boolean {
  return error instanceof TypeError
    || (error instanceof ApiError && [502, 503, 504].includes(error.status));
}
