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
export declare class OpenAssetClient {
    private readonly fetcher;
    private readonly baseUrl;
    private token?;
    constructor(baseUrl: string, token?: string, fetcher?: typeof fetch);
    setToken(token: string): void;
    login(username: string, password: string): Promise<UserSession>;
    listAdapters(): Promise<AdapterDefinition[]>;
    detectProject(request: DetectProjectRequest): Promise<ProjectDetection[]>;
    scanDependencies(request: AdapterScanRequest): Promise<DependencyScanResult>;
    generatePreview(request: AdapterScanRequest): Promise<PreviewGeneration>;
    extractMetadata(request: AdapterScanRequest): Promise<ExtractedMetadata>;
    validateAdapterChangelist(request: AdapterValidateRequest): Promise<AdapterValidationResponse>;
    listDepots(): Promise<Depot[]>;
    createDepot(name: string, description?: string): Promise<Depot>;
    listStreams(): Promise<Stream[]>;
    createStream(depot: string, name: string): Promise<Stream>;
    listWorkspaces(): Promise<Workspace[]>;
    createWorkspace(request: CreateWorkspaceRequest): Promise<Workspace>;
    deleteWorkspace(workspaceId: string): Promise<DeleteWorkspaceResponse>;
    listLocksPage(query?: {
        limit?: number;
        before_created_at?: string;
        before_id?: string;
        stream_id?: string;
        workspace_id?: string;
        user_id?: string;
    }): Promise<LockPage>;
    listLocks(): Promise<LockInfo[]>;
    listAuditPage(query?: {
        limit?: number;
        before_created_at?: string;
        before_id?: string;
        event_type?: string;
        actor_user_id?: string;
        stream_id?: string;
        workspace_id?: string;
    }): Promise<AuditPage>;
    listDependenciesPage(query: {
        stream_id: string;
        source_file?: string;
        limit?: number;
        before_scan_time?: string;
        before_id?: string;
    }): Promise<DependencyPage>;
    lockFile(workspaceId: string, path: string, reason?: string): Promise<LockInfo>;
    unlockFile(workspaceId: string, path: string): Promise<LockInfo>;
    createChangelist(workspaceId: string, description: string): Promise<Changelist>;
    addFile(request: FileOperationRequest): Promise<FileOperationResponse>;
    editFile(request: FileOperationRequest): Promise<FileOperationResponse>;
    deleteFile(request: FileOperationRequest): Promise<FileOperationResponse>;
    revertFile(request: FileOperationRequest): Promise<FileOperationResponse>;
    planSync(workspaceId: string, pathPrefix?: string): Promise<SyncPlanEntry[]>;
    fileHistory(workspaceId: string, path: string, limit?: number): Promise<FileHistoryEntry[]>;
    submitFiles(workspaceId: string, changelistId: string, files: SubmitFile[]): Promise<SubmitResponse>;
    private fileOperation;
    private authedRequest;
    private idempotencyHeaders;
    private requireToken;
    private request;
}
//# sourceMappingURL=index.d.ts.map