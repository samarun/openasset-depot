export type ViewKey =
  | "home"
  | "workspace"
  | "changes"
  | "shelves"
  | "reviews"
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

export interface SignupStatus {
  enabled: boolean;
  first_user: boolean;
}

export interface SsoStatus {
  enabled: boolean;
  /** Provider host, used to label the button without hard-coding a vendor. */
  provider?: string | null;
}

export interface SsoStart {
  authorization_url: string;
  state: string;
}

export interface UserAccount {
  id: string;
  username: string;
  display_name?: string | null;
  is_admin: boolean;
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
  preview_available?: boolean;
  review_proxy_available?: boolean;
}

export interface FileHistoryEntry {
  revision_number: number;
  blob_hash: string;
  size_bytes: number;
  action: string;
  submitted_by: string;
  submitted_at: string;
}

export type AnnotationTool = "pen" | "highlighter" | "rectangle" | "arrow";

export interface AnnotationPoint {
  x: number;
  y: number;
}

export interface ReviewAnnotation {
  tool: AnnotationTool;
  color: string;
  width: number;
  points: AnnotationPoint[];
}

export interface ReviewAnnotationPayload {
  marks: ReviewAnnotation[];
}

export interface ReviewComment {
  id: string;
  path: string;
  revision_number: number;
  author_user_id: string;
  author: string;
  parent_comment_id?: string | null;
  body: string;
  timecode_ms?: number | null;
  frame_number?: number | null;
  annotation?: ReviewAnnotationPayload | null;
  resolved_at?: string | null;
  resolved_by?: string | null;
  created_at: string;
}

export interface CreateReviewCommentInput {
  workspace_id: string;
  path: string;
  revision_number: number;
  body: string;
  timecode_ms?: number;
  frame_number?: number;
  parent_comment_id?: string;
  annotation?: ReviewAnnotationPayload;
}

export interface ReviewMedia {
  blob: Blob;
  contentType: string;
  source: "asset" | "preview";
}

export interface FileOperationResponse {
  path: string;
  action: "add" | "edit" | "delete" | "revert";
}

export interface ChangelistResponse {
  id: string;
  user_id: string;
  workspace_id: string;
  description: string;
  status: string;
  created_at: string;
}

export interface SubmittedRevision {
  path: string;
  revision_number: number;
  blob_hash: string;
  size_bytes: number;
}

export interface SubmitResponse {
  changelist_id: string;
  revisions: SubmittedRevision[];
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

export interface DependencyImpactEdge {
  /** The other file in the relationship, whichever direction it was reported for. */
  path: string;
  dependency_type: AssetDependency["dependency_type"];
  dependency_status: AssetDependency["dependency_status"];
  adapter_name: string;
  confidence: number;
  scan_time: string;
  in_depot: boolean;
}

export interface DependencyImpact {
  path: string;
  /** Files that reference this one, so changing it can break them. */
  required_by: DependencyImpactEdge[];
  /** Files this one references and needs present to open cleanly. */
  depends_on: DependencyImpactEdge[];
  required_by_count: number;
  depends_on_count: number;
  missing_count: number;
  truncated: boolean;
  /** `null` means the graph has never seen this file, not that it has no links. */
  last_scanned_at: string | null;
}

export interface ShelvedFile {
  path: string;
  blob_hash: string;
  size_bytes: number;
  action: "add" | "edit";
  created_at: string;
}

export interface Shelf {
  changelist_id: string;
  workspace_id: string;
  user_id: string;
  files: ShelvedFile[];
}

export interface ShelfSummary {
  changelist_id: string;
  description: string;
  changelist_status: string;
  user_id: string;
  owner: string;
  file_count: number;
  total_bytes: number;
  shelved_at: string;
}

export interface DiscardShelfResponse {
  changelist_id: string;
  discarded: number;
}

export interface ReviewerDecision {
  reviewer_user_id: string;
  reviewer: string;
  decision: "pending" | "approved" | "changes_requested";
  note: string | null;
  decided_at: string | null;
}

export interface ReviewRequest {
  id: string;
  path: string;
  revision_number: number;
  requested_by: string;
  requester: string;
  title: string;
  description: string;
  state: "open" | "approved" | "changes_requested" | "closed";
  created_at: string;
  updated_at: string;
  closed_at: string | null;
  reviewers: ReviewerDecision[];
}

export interface CreateReviewRequestInput {
  workspaceId: string;
  path: string;
  revisionNumber: number;
  title: string;
  description?: string;
  reviewers: string[];
}

export interface Collaborator {
  user_id: string;
  username: string;
  display_name?: string | null;
  role: string;
  is_admin: boolean;
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
  previewAvailable?: boolean;
  reviewProxyAvailable?: boolean;
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
