export type ViewKey =
  | "home"
  | "workspace"
  | "changes"
  | "locks"
  | "admin"
  | "settings"
  | "history";

export type StatusKind =
  | "Up to Date"
  | "Needs Sync"
  | "Checked Out"
  | "In Use"
  | "Ready to Submit"
  | "Blocked"
  | "Marked for Delete"
  | "New File";

export type SourceKind = "backend" | "mock";

export interface UserSession {
  token: string;
  username: string;
  serverUrl: string;
  isAdmin: boolean;
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

export interface CreateWorkspaceInput {
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

export interface Workspace {
  id: string;
  user_id?: string;
  depot_id: string;
  stream_id: string;
  name: string;
  local_path: string;
}

export interface LockInfo {
  id?: string;
  stream_id?: string;
  workspace_id?: string;
  user_id: string;
  depot_path: string;
  reason?: string | null;
  state: string;
  created_at?: string;
  source?: SourceKind;
}

export interface LockPageResponse {
  items: LockInfo[];
  next_before_created_at?: string | null;
  next_before_id?: string | null;
}

export interface FileTypeRule {
  id?: string;
  name: string;
  rule_kind: string;
  extension?: string | null;
  directory_prefix?: string | null;
  asset_class: string;
  is_binary?: boolean;
  lock_required: boolean;
  large_file?: boolean;
  generated: boolean;
  source?: SourceKind;
}

export interface ValidationMessage {
  path: string;
  code: string;
  message: string;
}

export interface ValidationResponse {
  warnings: ValidationMessage[];
  errors: ValidationMessage[];
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

export interface FileOperationResponse {
  path: string;
  action: "add" | "edit" | "delete" | "revert";
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

export interface DependencyScanResult {
  source_file: string;
  adapter_name: string;
  dependencies: AssetDependency[];
}

export interface AssetDependency {
  source_file: string;
  target_file: string;
  dependency_type: "metadata" | "reference" | "texture" | "plate" | "lut" | "media" | "unknown";
  dependency_status: "present" | "missing" | "external" | "unknown";
  adapter_name: string;
  confidence: number;
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

export interface AssetFile {
  id: string;
  name: string;
  path: string;
  kind: string;
  size: string;
  revision: number;
  owner?: string;
  statuses: StatusKind[];
  previewTone: "image" | "model" | "audio" | "scene" | "document";
  dependencies: string[];
  changelist?: string;
  source: SourceKind;
}

export interface Changelist {
  id: string;
  title: string;
  description: string;
  files: AssetFile[];
  warnings: ValidationMessage[];
  ready: boolean;
  source: SourceKind;
}

export interface Review {
  id: string;
  title: string;
  status: "Waiting" | "Approved" | "Changes Requested";
  owner: string;
  files: number;
  source: SourceKind;
}

export interface StageState {
  stageRevision: string;
  liveApproval: "Pending" | "Approved" | "Blocked";
  renderNodes: Array<{ name: string; status: StatusKind; lastSync: string }>;
  validations: ValidationMessage[];
  source: SourceKind;
}
