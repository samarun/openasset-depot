export interface UserSession {
  token: string;
  user: {
    username: string;
  };
}

export interface Depot {
  id: string;
  name: string;
  description?: string | null;
}

export interface Stream {
  id: string;
  depot_id: string;
  name: string;
}

export interface Workspace {
  id: string;
  user_id: string;
  depot_id: string;
  stream_id: string;
  name: string;
  local_path: string;
}

export interface CreateWorkspaceRequest {
  name: string;
  depot: string;
  stream: string;
  local_path: string;
}

export interface DeleteWorkspaceResponse {
  id: string;
  deleted: boolean;
  released_locks: number;
  abandoned_changelists: number;
}

export interface LockInfo {
  id: string;
  stream_id: string;
  workspace_id: string;
  user_id: string;
  depot_path: string;
  reason?: string | null;
  state: string;
  created_at: string;
}

export interface LockPage {
  items: LockInfo[];
  next_before_created_at?: string | null;
  next_before_id?: string | null;
}

export interface AuditEvent {
  id: string;
  event_type: string;
  actor_user_id?: string | null;
  stream_id?: string | null;
  workspace_id?: string | null;
  depot_path?: string | null;
  changelist_id?: string | null;
  details: Record<string, unknown>;
  created_at: string;
}

export interface AuditPage {
  items: AuditEvent[];
  next_before_created_at?: string | null;
  next_before_id?: string | null;
}

export interface Changelist {
  id: string;
  user_id: string;
  workspace_id: string;
  description: string;
  status: string;
  created_at: string;
}

export interface FileOperationRequest {
  workspace_id: string;
  changelist_id?: string;
  path: string;
}

export interface FileOperationResponse {
  path: string;
  action: "add" | "edit" | "delete" | "revert";
}

export interface SyncPlanEntry {
  path: string;
  revision_number: number;
  blob_hash: string;
  size_bytes: number;
  deleted: boolean;
}

export interface FileHistoryEntry {
  revision_number: number;
  blob_hash: string;
  size_bytes: number;
  action: string;
  submitted_by: string;
  submitted_at: string;
}

export interface SubmitFile {
  path: string;
  blob: Blob;
}

export interface SubmitResponse {
  changelist_id: string;
  revisions: Array<{
    path: string;
    revision_number: number;
    blob_hash: string;
    size_bytes: number;
  }>;
}

export interface AdapterDefinition {
  app_name: string;
  supported_file_types: string[];
  project_detection_rules: string[];
  scene_file_patterns: string[];
  asset_file_patterns: string[];
  lock_required_patterns: string[];
  ignored_patterns: string[];
  generated_cache_patterns: string[];
  dependency_scan_strategy: string;
  preview_generation_strategy: string;
  validation_rules: string[];
  metadata_extraction_rules: string[];
}

export interface DetectProjectRequest {
  root_path: string;
  relative_paths?: string[];
}

export interface ProjectDetection {
  app_name: string;
  root_path: string;
  matched_rule: string;
  confidence: number;
}

export interface AdapterFileInput {
  path: string;
  content?: string;
  size_bytes?: number;
}

export interface AdapterScanRequest {
  adapter_name?: string;
  workspace_id?: string;
  file: AdapterFileInput;
}

export interface DependencyScanResult {
  source_file: string;
  adapter_name: string;
  dependencies: AssetDependency[];
}

export interface AssetDependency {
  source_file: string;
  target_file: string;
  dependency_type: DependencyType;
  dependency_status: DependencyStatus;
  adapter_name: string;
  confidence: number;
}

export type DependencyType = "metadata" | "reference" | "texture" | "plate" | "lut" | "media" | "unknown";
export type DependencyStatus = "present" | "missing" | "external" | "unknown";

export interface DependencyEdge {
  id: string;
  stream_id: string;
  source_file: string;
  target_file: string;
  adapter_name: string;
  dependency_type: DependencyType;
  dependency_status: DependencyStatus;
  scan_time: string;
  confidence: number;
  metadata: Record<string, unknown>;
}

export interface DependencyPage {
  items: DependencyEdge[];
  next_before_scan_time?: string | null;
  next_before_id?: string | null;
}

export interface AdapterValidateRequest {
  adapter_name?: string;
  files: AdapterFileInput[];
}

export interface AdapterValidationResponse {
  warnings: AdapterValidationMessage[];
  errors: AdapterValidationMessage[];
}

export interface AdapterValidationMessage {
  path: string;
  code: string;
  severity: "warning" | "error";
  message: string;
  adapter_name: string;
}

export interface PreviewGeneration {
  file: string;
  adapter_name: string;
  strategy: string;
  status: "not_generated" | "external_tool_required" | "unsupported";
  message: string;
}

export interface ExtractedMetadata {
  file: string;
  adapter_name: string;
  metadata: Record<string, string>;
}

export class OpenAssetClient {
  private readonly baseUrl: string;
  private token?: string;

  constructor(baseUrl: string, token?: string, private readonly fetcher: typeof fetch = fetch) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.token = token;
  }

  setToken(token: string): void {
    this.token = token;
  }

  async login(username: string, password: string): Promise<UserSession> {
    const session = await this.request<UserSession>("/api/auth/login", {
      method: "POST",
      body: JSON.stringify({ username, password }),
    });
    this.token = session.token;
    return session;
  }

  listAdapters(): Promise<AdapterDefinition[]> {
    return this.request<AdapterDefinition[]>("/api/adapters", { token: this.requireToken() });
  }

  detectProject(request: DetectProjectRequest): Promise<ProjectDetection[]> {
    return this.request<ProjectDetection[]>("/api/adapters/detect", {
      method: "POST",
      token: this.requireToken(),
      body: JSON.stringify(request),
    });
  }

  scanDependencies(request: AdapterScanRequest): Promise<DependencyScanResult> {
    return this.request<DependencyScanResult>("/api/adapters/scan", {
      method: "POST",
      token: this.requireToken(),
      body: JSON.stringify(request),
    });
  }

  generatePreview(request: AdapterScanRequest): Promise<PreviewGeneration> {
    return this.request<PreviewGeneration>("/api/adapters/preview", {
      method: "POST",
      token: this.requireToken(),
      body: JSON.stringify(request),
    });
  }

  extractMetadata(request: AdapterScanRequest): Promise<ExtractedMetadata> {
    return this.request<ExtractedMetadata>("/api/adapters/metadata", {
      method: "POST",
      token: this.requireToken(),
      body: JSON.stringify(request),
    });
  }

  validateAdapterChangelist(request: AdapterValidateRequest): Promise<AdapterValidationResponse> {
    return this.request<AdapterValidationResponse>("/api/adapters/validate", {
      method: "POST",
      token: this.requireToken(),
      body: JSON.stringify(request),
    });
  }

  listDepots(): Promise<Depot[]> {
    return this.authedRequest<Depot[]>("/api/depots");
  }

  createDepot(name: string, description?: string): Promise<Depot> {
    return this.authedRequest<Depot>("/api/depots", {
      method: "POST",
      headers: this.idempotencyHeaders(),
      body: JSON.stringify({ name, description }),
    });
  }

  listStreams(): Promise<Stream[]> {
    return this.authedRequest<Stream[]>("/api/streams");
  }

  createStream(depot: string, name: string): Promise<Stream> {
    return this.authedRequest<Stream>("/api/streams", {
      method: "POST",
      headers: this.idempotencyHeaders(),
      body: JSON.stringify({ depot, name }),
    });
  }

  listWorkspaces(): Promise<Workspace[]> {
    return this.authedRequest<Workspace[]>("/api/workspaces");
  }

  createWorkspace(request: CreateWorkspaceRequest): Promise<Workspace> {
    return this.authedRequest<Workspace>("/api/workspaces", {
      method: "POST",
      headers: this.idempotencyHeaders(),
      body: JSON.stringify(request),
    });
  }

  deleteWorkspace(workspaceId: string): Promise<DeleteWorkspaceResponse> {
    return this.authedRequest<DeleteWorkspaceResponse>(`/api/workspaces/${workspaceId}`, {
      method: "DELETE",
    });
  }

  listLocksPage(query: {
    limit?: number;
    before_created_at?: string;
    before_id?: string;
    stream_id?: string;
    workspace_id?: string;
    user_id?: string;
  } = {}): Promise<LockPage> {
    const params = queryParams(query);
    return this.authedRequest<LockPage>(`/api/locks/page?${params}`);
  }

  async listLocks(): Promise<LockInfo[]> {
    const locks: LockInfo[] = [];
    let before_created_at: string | undefined;
    let before_id: string | undefined;
    for (;;) {
      const page = await this.listLocksPage({ limit: 1000, before_created_at, before_id });
      locks.push(...page.items);
      const nextCreatedAt = page.next_before_created_at ?? undefined;
      const nextId = page.next_before_id ?? undefined;
      if (!nextCreatedAt || !nextId) return locks;
      if (nextCreatedAt === before_created_at && nextId === before_id) {
        throw new Error("Server returned a repeated lock-page cursor");
      }
      before_created_at = nextCreatedAt;
      before_id = nextId;
    }
  }

  listAuditPage(query: {
    limit?: number;
    before_created_at?: string;
    before_id?: string;
    event_type?: string;
    actor_user_id?: string;
    stream_id?: string;
    workspace_id?: string;
  } = {}): Promise<AuditPage> {
    const params = queryParams(query);
    return this.authedRequest<AuditPage>(`/api/audit?${params}`);
  }

  listDependenciesPage(query: {
    stream_id: string;
    source_file?: string;
    limit?: number;
    before_scan_time?: string;
    before_id?: string;
  }): Promise<DependencyPage> {
    const params = queryParams(query);
    return this.authedRequest<DependencyPage>(`/api/dependencies?${params}`);
  }

  lockFile(workspaceId: string, path: string, reason?: string): Promise<LockInfo> {
    return this.authedRequest<LockInfo>("/api/files/lock", {
      method: "POST",
      body: JSON.stringify({ workspace_id: workspaceId, path, reason }),
    });
  }

  unlockFile(workspaceId: string, path: string): Promise<LockInfo> {
    return this.authedRequest<LockInfo>("/api/files/unlock", {
      method: "POST",
      body: JSON.stringify({ workspace_id: workspaceId, path }),
    });
  }

  createChangelist(workspaceId: string, description: string): Promise<Changelist> {
    return this.authedRequest<Changelist>("/api/changelists", {
      method: "POST",
      headers: this.idempotencyHeaders(),
      body: JSON.stringify({ workspace_id: workspaceId, description }),
    });
  }

  addFile(request: FileOperationRequest): Promise<FileOperationResponse> {
    return this.fileOperation("/api/files/add", request);
  }

  editFile(request: FileOperationRequest): Promise<FileOperationResponse> {
    return this.fileOperation("/api/files/edit", request);
  }

  deleteFile(request: FileOperationRequest): Promise<FileOperationResponse> {
    return this.fileOperation("/api/files/delete", request);
  }

  revertFile(request: FileOperationRequest): Promise<FileOperationResponse> {
    return this.fileOperation("/api/files/revert", request);
  }

  planSync(workspaceId: string, pathPrefix?: string): Promise<SyncPlanEntry[]> {
    return this.authedRequest<SyncPlanEntry[]>("/api/sync/plan", {
      method: "POST",
      body: JSON.stringify({ workspace_id: workspaceId, path_prefix: pathPrefix }),
    });
  }

  fileHistory(workspaceId: string, path: string, limit = 100): Promise<FileHistoryEntry[]> {
    const query = new URLSearchParams({
      workspace_id: workspaceId,
      path,
      limit: String(limit),
    });
    return this.authedRequest<FileHistoryEntry[]>(`/api/files/history?${query.toString()}`);
  }

  submitFiles(
    workspaceId: string,
    changelistId: string,
    files: SubmitFile[],
  ): Promise<SubmitResponse> {
    if (files.length === 0) throw new Error("Submit requires at least one file");
    const form = new FormData();
    form.set("workspace_id", workspaceId);
    for (const file of files) form.append("file", file.blob, file.path);
    return this.authedRequest<SubmitResponse>(`/api/changelists/${changelistId}/submit`, {
      method: "POST",
      body: form,
    });
  }

  private fileOperation(
    endpoint: string,
    request: FileOperationRequest,
  ): Promise<FileOperationResponse> {
    return this.authedRequest<FileOperationResponse>(endpoint, {
      method: "POST",
      body: JSON.stringify(request),
    });
  }

  private authedRequest<T>(path: string, options: RequestInit = {}): Promise<T> {
    return this.request<T>(path, { ...options, token: this.requireToken() });
  }

  private idempotencyHeaders(): HeadersInit {
    return { "idempotency-key": crypto.randomUUID() };
  }

  private requireToken(): string {
    if (!this.token) throw new Error("OpenAssetClient requires a token for this endpoint");
    return this.token;
  }

  private async request<T>(path: string, options: RequestInit & { token?: string } = {}): Promise<T> {
    const headers = new Headers(options.headers);
    if (!(options.body instanceof FormData)) headers.set("content-type", "application/json");
    if (options.token) headers.set("authorization", `Bearer ${options.token}`);

    const response = await this.fetcher(`${this.baseUrl}${path}`, { ...options, headers });
    if (!response.ok) {
      const body = await response.text();
      throw new Error(`OpenAsset request failed with ${response.status}: ${body}`);
    }
    return (await response.json()) as T;
  }
}

function queryParams(query: object): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== null) params.set(key, String(value));
  }
  return params.toString();
}
