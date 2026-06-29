use std::path::PathBuf;

use reqwest::{multipart, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Clone)]
pub struct OpenAssetClient {
    base_url: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl OpenAssetClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: None,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub async fn login(&mut self, username: &str, password: &str) -> Result<LoginResponse> {
        let response: LoginResponse = self
            .post(
                "/api/auth/login",
                &serde_json::json!({ "username": username, "password": password }),
            )
            .await?;
        self.token = Some(response.token.clone());
        Ok(response)
    }

    pub async fn list_adapters(&self) -> Result<Vec<AdapterDefinition>> {
        self.get("/api/adapters").await
    }

    pub async fn detect_project(
        &self,
        req: &DetectProjectRequest,
    ) -> Result<Vec<ProjectDetection>> {
        self.authed_post("/api/adapters/detect", req).await
    }

    pub async fn scan_dependencies(
        &self,
        req: &AdapterScanRequest,
    ) -> Result<DependencyScanResult> {
        self.authed_post("/api/adapters/scan", req).await
    }

    pub async fn generate_preview(&self, req: &AdapterScanRequest) -> Result<PreviewGeneration> {
        self.authed_post("/api/adapters/preview", req).await
    }

    pub async fn extract_metadata(&self, req: &AdapterScanRequest) -> Result<ExtractedMetadata> {
        self.authed_post("/api/adapters/metadata", req).await
    }

    pub async fn validate_adapter_changelist(
        &self,
        req: &AdapterValidateRequest,
    ) -> Result<AdapterValidationResponse> {
        self.authed_post("/api/adapters/validate", req).await
    }

    pub async fn list_depots(&self) -> Result<Vec<Depot>> {
        self.get("/api/depots").await
    }

    pub async fn create_depot(&self, name: &str, description: Option<&str>) -> Result<Depot> {
        self.authed_post_idempotent(
            "/api/depots",
            &serde_json::json!({ "name": name, "description": description }),
        )
        .await
    }

    pub async fn list_streams(&self) -> Result<Vec<Stream>> {
        self.get("/api/streams").await
    }

    pub async fn create_stream(&self, depot: &str, name: &str) -> Result<Stream> {
        self.authed_post_idempotent(
            "/api/streams",
            &serde_json::json!({ "depot": depot, "name": name }),
        )
        .await
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.get("/api/workspaces").await
    }

    pub async fn create_workspace(&self, request: &CreateWorkspaceRequest) -> Result<Workspace> {
        self.authed_post_idempotent("/api/workspaces", request)
            .await
    }

    pub async fn delete_workspace(&self, workspace_id: Uuid) -> Result<DeleteWorkspaceResponse> {
        let request = self
            .client
            .delete(self.url(&format!("/api/workspaces/{workspace_id}")));
        self.send_json(self.authorize(request)?).await
    }

    pub async fn list_locks_page(&self, query: &LockPageQuery) -> Result<LockPage> {
        self.get(&format!("/api/locks/page?{}", query.to_query()))
            .await
    }

    pub async fn list_locks(&self) -> Result<Vec<Lock>> {
        let mut locks = Vec::new();
        let mut before_created_at = None;
        let mut before_id = None;
        loop {
            let page = self
                .list_locks_page(&LockPageQuery {
                    limit: Some(1_000),
                    before_created_at: before_created_at.clone(),
                    before_id,
                    ..LockPageQuery::default()
                })
                .await?;
            locks.extend(page.items);
            let next_created_at = page.next_before_created_at;
            let next_id = page.next_before_id;
            if next_created_at.is_none() || next_id.is_none() {
                return Ok(locks);
            }
            if next_created_at == before_created_at && next_id == before_id {
                return Err(OpenAssetSdkError::InvalidInput(
                    "server returned a repeated lock-page cursor".to_string(),
                ));
            }
            before_created_at = next_created_at;
            before_id = next_id;
        }
    }

    pub async fn list_audit_page(&self, query: &AuditPageQuery) -> Result<AuditPage> {
        self.get(&format!("/api/audit?{}", query.to_query())).await
    }

    pub async fn list_dependencies_page(
        &self,
        query: &DependencyPageQuery,
    ) -> Result<DependencyPage> {
        self.get(&format!("/api/dependencies?{}", query.to_query()))
            .await
    }

    pub async fn lock_file(&self, request: &LockRequest) -> Result<Lock> {
        self.authed_post("/api/files/lock", request).await
    }

    pub async fn unlock_file(&self, workspace_id: Uuid, path: &str) -> Result<Lock> {
        self.authed_post(
            "/api/files/unlock",
            &serde_json::json!({ "workspace_id": workspace_id, "path": path }),
        )
        .await
    }

    pub async fn create_changelist(
        &self,
        workspace_id: Uuid,
        description: &str,
    ) -> Result<Changelist> {
        self.authed_post_idempotent(
            "/api/changelists",
            &serde_json::json!({
                "workspace_id": workspace_id,
                "description": description,
            }),
        )
        .await
    }

    pub async fn add_file(&self, request: &FileOperationRequest) -> Result<FileOperationResponse> {
        self.authed_post("/api/files/add", request).await
    }

    pub async fn edit_file(&self, request: &FileOperationRequest) -> Result<FileOperationResponse> {
        self.authed_post("/api/files/edit", request).await
    }

    pub async fn delete_file(
        &self,
        request: &FileOperationRequest,
    ) -> Result<FileOperationResponse> {
        self.authed_post("/api/files/delete", request).await
    }

    pub async fn revert_file(
        &self,
        request: &FileOperationRequest,
    ) -> Result<FileOperationResponse> {
        self.authed_post("/api/files/revert", request).await
    }

    pub async fn plan_sync(&self, workspace_id: Uuid) -> Result<Vec<SyncPlanEntry>> {
        self.authed_post(
            "/api/sync/plan",
            &serde_json::json!({ "workspace_id": workspace_id }),
        )
        .await
    }

    pub async fn file_history(
        &self,
        workspace_id: Uuid,
        path: &str,
        limit: u32,
    ) -> Result<Vec<FileHistoryEntry>> {
        let query = format!(
            "/api/files/history?workspace_id={workspace_id}&path={}&limit={limit}",
            urlencoding::encode(path)
        );
        self.get(&query).await
    }

    pub async fn submit_files(
        &self,
        workspace_id: Uuid,
        changelist_id: Uuid,
        files: Vec<SubmitFile>,
    ) -> Result<SubmitResponse> {
        if files.is_empty() {
            return Err(OpenAssetSdkError::InvalidInput(
                "submit requires at least one file".to_string(),
            ));
        }
        let mut form = multipart::Form::new().text("workspace_id", workspace_id.to_string());
        for submission in files {
            let file = tokio::fs::File::open(&submission.local_path)
                .await
                .map_err(OpenAssetSdkError::Io)?;
            let stream = ReaderStream::new(file);
            let part = multipart::Part::stream(reqwest::Body::wrap_stream(stream))
                .file_name(submission.depot_path);
            form = form.part("file", part);
        }
        let request = self
            .client
            .post(self.url(&format!("/api/changelists/{changelist_id}/submit")))
            .multipart(form);
        self.send_json(self.authorize(request)?).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let request = self.client.get(self.url(path));
        self.send_json(self.authorize(request)?).await
    }

    async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.send_json(self.client.post(self.url(path)).json(body))
            .await
    }

    async fn authed_post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let request = self.client.post(self.url(path)).json(body);
        self.send_json(self.authorize(request)?).await
    }

    async fn authed_post_idempotent<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let request = self
            .client
            .post(self.url(path))
            .header("Idempotency-Key", Uuid::new_v4().to_string())
            .json(body);
        self.send_json(self.authorize(request)?).await
    }

    async fn send_json<T: DeserializeOwned>(&self, request: reqwest::RequestBuilder) -> Result<T> {
        let response = request.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response.json::<T>().await?);
        }
        let body = response.text().await.unwrap_or_default();
        Err(OpenAssetSdkError::Http { status, body })
    }

    fn authorize(&self, request: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
        self.token
            .as_ref()
            .map(|token| request.bearer_auth(token))
            .ok_or(OpenAssetSdkError::MissingToken)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

pub type Result<T> = std::result::Result<T, OpenAssetSdkError>;

#[derive(Debug)]
pub enum OpenAssetSdkError {
    MissingToken,
    InvalidInput(String),
    Http { status: StatusCode, body: String },
    Transport(reqwest::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for OpenAssetSdkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingToken => write!(f, "missing auth token"),
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::Http { status, body } => write!(f, "request failed with {status}: {body}"),
            Self::Transport(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for OpenAssetSdkError {}

impl From<reqwest::Error> for OpenAssetSdkError {
    fn from(value: reqwest::Error) -> Self {
        Self::Transport(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Depot {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub id: Uuid,
    pub depot_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub depot: String,
    pub stream: String,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub user_id: Uuid,
    pub depot_id: Uuid,
    pub stream_id: Uuid,
    pub name: String,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteWorkspaceResponse {
    pub id: Uuid,
    pub deleted: bool,
    pub released_locks: u64,
    pub abandoned_changelists: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRequest {
    pub workspace_id: Uuid,
    pub path: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lock {
    pub id: Uuid,
    pub stream_id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub depot_path: String,
    pub reason: Option<String>,
    pub state: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct LockPageQuery {
    pub limit: Option<u16>,
    pub before_created_at: Option<String>,
    pub before_id: Option<Uuid>,
    pub stream_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
}

impl LockPageQuery {
    fn to_query(&self) -> String {
        query_string([
            ("limit", self.limit.map(|value| value.to_string())),
            ("before_created_at", self.before_created_at.clone()),
            ("before_id", self.before_id.map(|value| value.to_string())),
            ("stream_id", self.stream_id.map(|value| value.to_string())),
            (
                "workspace_id",
                self.workspace_id.map(|value| value.to_string()),
            ),
            ("user_id", self.user_id.map(|value| value.to_string())),
        ])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockPage {
    pub items: Vec<Lock>,
    pub next_before_created_at: Option<String>,
    pub next_before_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct AuditPageQuery {
    pub limit: Option<u16>,
    pub before_created_at: Option<String>,
    pub before_id: Option<Uuid>,
    pub event_type: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub stream_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
}

impl AuditPageQuery {
    fn to_query(&self) -> String {
        query_string([
            ("limit", self.limit.map(|value| value.to_string())),
            ("before_created_at", self.before_created_at.clone()),
            ("before_id", self.before_id.map(|value| value.to_string())),
            ("event_type", self.event_type.clone()),
            (
                "actor_user_id",
                self.actor_user_id.map(|value| value.to_string()),
            ),
            ("stream_id", self.stream_id.map(|value| value.to_string())),
            (
                "workspace_id",
                self.workspace_id.map(|value| value.to_string()),
            ),
        ])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub event_type: String,
    pub actor_user_id: Option<Uuid>,
    pub stream_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub depot_path: Option<String>,
    pub changelist_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPage {
    pub items: Vec<AuditEvent>,
    pub next_before_created_at: Option<String>,
    pub next_before_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changelist {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub description: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperationRequest {
    pub workspace_id: Uuid,
    pub changelist_id: Option<Uuid>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperationResponse {
    pub path: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPlanEntry {
    pub path: String,
    pub revision_number: i32,
    pub blob_hash: String,
    pub size_bytes: i64,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryEntry {
    pub revision_number: i32,
    pub blob_hash: String,
    pub size_bytes: i64,
    pub action: String,
    pub submitted_by: Uuid,
    pub submitted_at: String,
}

#[derive(Debug, Clone)]
pub struct SubmitFile {
    pub depot_path: String,
    pub local_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitResponse {
    pub changelist_id: Uuid,
    pub revisions: Vec<SubmittedRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmittedRevision {
    pub path: String,
    pub revision_number: i32,
    pub blob_hash: String,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterDefinition {
    pub app_name: String,
    pub supported_file_types: Vec<String>,
    pub project_detection_rules: Vec<String>,
    pub scene_file_patterns: Vec<String>,
    pub asset_file_patterns: Vec<String>,
    pub lock_required_patterns: Vec<String>,
    pub ignored_patterns: Vec<String>,
    pub generated_cache_patterns: Vec<String>,
    pub dependency_scan_strategy: String,
    pub preview_generation_strategy: String,
    pub validation_rules: Vec<String>,
    pub metadata_extraction_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectProjectRequest {
    pub root_path: String,
    #[serde(default)]
    pub relative_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetection {
    pub app_name: String,
    pub root_path: String,
    pub matched_rule: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterFileInput {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterScanRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,
    pub file: AdapterFileInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyScanResult {
    pub source_file: String,
    pub adapter_name: String,
    pub dependencies: Vec<AssetDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDependency {
    pub source_file: String,
    pub target_file: String,
    pub dependency_type: DependencyType,
    pub dependency_status: DependencyStatus,
    pub adapter_name: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Metadata,
    Reference,
    Texture,
    Plate,
    Lut,
    Media,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Present,
    Missing,
    External,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DependencyPageQuery {
    pub stream_id: Uuid,
    pub source_file: Option<String>,
    pub limit: Option<u16>,
    pub before_scan_time: Option<String>,
    pub before_id: Option<Uuid>,
}

impl DependencyPageQuery {
    fn to_query(&self) -> String {
        query_string([
            ("stream_id", Some(self.stream_id.to_string())),
            ("source_file", self.source_file.clone()),
            ("limit", self.limit.map(|value| value.to_string())),
            ("before_scan_time", self.before_scan_time.clone()),
            ("before_id", self.before_id.map(|value| value.to_string())),
        ])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub id: Uuid,
    pub stream_id: Uuid,
    pub source_file: String,
    pub target_file: String,
    pub adapter_name: String,
    pub dependency_type: DependencyType,
    pub dependency_status: DependencyStatus,
    pub scan_time: String,
    pub confidence: f64,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyPage {
    pub items: Vec<DependencyEdge>,
    pub next_before_scan_time: Option<String>,
    pub next_before_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterValidateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_name: Option<String>,
    pub files: Vec<AdapterFileInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterValidationResponse {
    pub warnings: Vec<AdapterValidationMessage>,
    pub errors: Vec<AdapterValidationMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterValidationMessage {
    pub path: String,
    pub code: String,
    pub severity: ValidationSeverity,
    pub message: String,
    pub adapter_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewGeneration {
    pub file: String,
    pub adapter_name: String,
    pub strategy: String,
    pub status: PreviewStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewStatus {
    NotGenerated,
    ExternalToolRequired,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMetadata {
    pub file: String,
    pub adapter_name: String,
    pub metadata: std::collections::BTreeMap<String, String>,
}

fn query_string<const N: usize>(pairs: [(&str, Option<String>); N]) -> String {
    pairs
        .into_iter()
        .filter_map(|(key, value)| {
            value.map(|value| format!("{key}={}", urlencoding::encode(&value)))
        })
        .collect::<Vec<_>>()
        .join("&")
}
