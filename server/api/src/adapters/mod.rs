use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, LazyLock},
};

use axum::{extract::State, http::HeaderMap, Json};
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    auth::AuthUser,
    error::{AppError, AppResult},
    paths::normalize_depot_path,
    workspaces::{workspace_for_user, WorkspaceContext},
};

static MAYA_REFERENCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""([^"]+\.(?:ma|mb|fbx|abc|usd|obj|exr|tif|tx|wav))""#)
        .expect("Maya reference regex must compile")
});

static NUKE_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)\b(?:file|proxy)\s+([^\s\]]+\.(?:exr|dpx|mov|cube|lut|png|jpg|jpeg|tif|tiff))"#,
    )
    .expect("Nuke path regex must compile")
});

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
pub struct ProjectProbe {
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
pub struct AdapterChangelist {
    pub files: Vec<AdapterFileInput>,
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

impl DependencyType {
    fn as_db_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Reference => "reference",
            Self::Texture => "texture",
            Self::Plate => "plate",
            Self::Lut => "lut",
            Self::Media => "media",
            Self::Unknown => "unknown",
        }
    }
}

impl DependencyStatus {
    fn as_db_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::External => "external",
            Self::Unknown => "unknown",
        }
    }
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
    pub metadata: BTreeMap<String, String>,
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

#[derive(Debug, Deserialize)]
pub struct DetectProjectRequest {
    pub root_path: String,
    #[serde(default)]
    pub relative_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdapterScanRequest {
    #[serde(default)]
    pub adapter_name: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<Uuid>,
    pub file: AdapterFileInput,
}

#[derive(Debug, Deserialize)]
pub struct AdapterValidateRequest {
    #[serde(default)]
    pub adapter_name: Option<String>,
    pub files: Vec<AdapterFileInput>,
}

#[derive(Debug, Serialize)]
pub struct AdapterValidationResponse {
    pub warnings: Vec<AdapterValidationMessage>,
    pub errors: Vec<AdapterValidationMessage>,
}

pub trait DccAdapter: Send + Sync {
    fn definition(&self) -> &AdapterDefinition;

    fn detect_project(&self, probe: &ProjectProbe) -> Option<ProjectDetection>;

    fn detect_open_file(&self, path: &str) -> bool;

    fn scan_dependencies(&self, file: &AdapterFileInput) -> DependencyScanResult;

    fn generate_preview(&self, file: &AdapterFileInput) -> PreviewGeneration;

    fn validate_before_submit(
        &self,
        changelist: &AdapterChangelist,
    ) -> Vec<AdapterValidationMessage>;

    fn extract_metadata(&self, file: &AdapterFileInput) -> ExtractedMetadata;

    fn get_lock_required_patterns(&self) -> &[String] {
        &self.definition().lock_required_patterns
    }

    fn get_generated_cache_patterns(&self) -> &[String] {
        &self.definition().generated_cache_patterns
    }

    fn get_ignored_patterns(&self) -> &[String] {
        &self.definition().ignored_patterns
    }
}

#[derive(Clone)]
pub struct AdapterRegistry {
    adapters: Vec<Arc<dyn DccAdapter>>,
    by_name: HashMap<String, Arc<dyn DccAdapter>>,
}

impl AdapterRegistry {
    pub fn new(adapters: Vec<Arc<dyn DccAdapter>>) -> Self {
        let by_name = adapters
            .iter()
            .map(|adapter| {
                (
                    adapter.definition().app_name.to_ascii_lowercase(),
                    Arc::clone(adapter),
                )
            })
            .collect();
        Self { adapters, by_name }
    }

    pub fn default_registry() -> Self {
        Self::new(vec![
            Arc::new(RuleBasedAdapter::new(unreal_definition(), ScanMode::None)),
            Arc::new(RuleBasedAdapter::new(
                unity_definition(),
                ScanMode::UnityMeta,
            )),
            Arc::new(RuleBasedAdapter::new(blender_definition(), ScanMode::None)),
            Arc::new(RuleBasedAdapter::new(
                maya_definition(),
                ScanMode::MayaAscii,
            )),
            Arc::new(RuleBasedAdapter::new(houdini_definition(), ScanMode::None)),
            Arc::new(RuleBasedAdapter::new(
                nuke_definition(),
                ScanMode::NukeScript,
            )),
            Arc::new(RuleBasedAdapter::new(premiere_definition(), ScanMode::None)),
            Arc::new(RuleBasedAdapter::new(
                after_effects_definition(),
                ScanMode::None,
            )),
            Arc::new(RuleBasedAdapter::new(
                photoshop_definition(),
                ScanMode::None,
            )),
            Arc::new(RuleBasedAdapter::new(resolve_definition(), ScanMode::None)),
            Arc::new(RuleBasedAdapter::new(
                generic_media_definition(),
                ScanMode::None,
            )),
        ])
    }

    pub fn definitions(&self) -> Vec<AdapterDefinition> {
        self.adapters
            .iter()
            .map(|adapter| adapter.definition().clone())
            .collect()
    }

    pub fn get(&self, app_name: &str) -> Option<Arc<dyn DccAdapter>> {
        self.by_name.get(&app_name.to_ascii_lowercase()).cloned()
    }

    pub fn detect_project(&self, probe: &ProjectProbe) -> Vec<ProjectDetection> {
        let mut detections: Vec<_> = self
            .adapters
            .iter()
            .filter_map(|adapter| adapter.detect_project(probe))
            .collect();
        detections.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        detections
    }

    pub fn detect_open_file(&self, path: &str) -> Option<String> {
        self.adapters
            .iter()
            .find(|adapter| adapter.detect_open_file(path))
            .map(|adapter| adapter.definition().app_name.clone())
    }

    pub fn scan_dependencies(
        &self,
        adapter_name: Option<&str>,
        file: &AdapterFileInput,
    ) -> AppResult<DependencyScanResult> {
        let adapter = self.resolve_adapter(adapter_name, &file.path)?;
        Ok(adapter.scan_dependencies(file))
    }

    pub fn validate_changelist(
        &self,
        adapter_name: Option<&str>,
        changelist: &AdapterChangelist,
    ) -> Vec<AdapterValidationMessage> {
        if let Some(name) = adapter_name {
            return self
                .get(name)
                .map(|adapter| adapter.validate_before_submit(changelist))
                .unwrap_or_default();
        }

        let mut messages = Vec::new();
        for adapter in &self.adapters {
            messages.extend(adapter.validate_before_submit(changelist));
        }
        dedupe_validation_messages(messages)
    }

    pub fn extract_metadata(
        &self,
        adapter_name: Option<&str>,
        file: &AdapterFileInput,
    ) -> AppResult<ExtractedMetadata> {
        let adapter = self.resolve_adapter(adapter_name, &file.path)?;
        Ok(adapter.extract_metadata(file))
    }

    pub fn generate_preview(
        &self,
        adapter_name: Option<&str>,
        file: &AdapterFileInput,
    ) -> AppResult<PreviewGeneration> {
        let adapter = self.resolve_adapter(adapter_name, &file.path)?;
        Ok(adapter.generate_preview(file))
    }

    fn resolve_adapter(
        &self,
        adapter_name: Option<&str>,
        path: &str,
    ) -> AppResult<Arc<dyn DccAdapter>> {
        if let Some(name) = adapter_name {
            return self
                .get(name)
                .ok_or_else(|| AppError::NotFound(format!("adapter not found: {name}")));
        }
        let detected = self
            .detect_open_file(path)
            .ok_or_else(|| AppError::NotFound(format!("no adapter supports path: {path}")))?;
        self.get(&detected)
            .ok_or_else(|| AppError::internal("detected adapter missing from registry"))
    }
}

#[derive(Debug, Clone, Copy)]
enum ScanMode {
    None,
    UnityMeta,
    MayaAscii,
    NukeScript,
}

struct RuleBasedAdapter {
    definition: AdapterDefinition,
    scan_mode: ScanMode,
    scene_patterns: GlobSet,
    asset_patterns: GlobSet,
    lock_patterns: GlobSet,
    ignored_patterns: GlobSet,
    generated_patterns: GlobSet,
}

impl RuleBasedAdapter {
    fn new(definition: AdapterDefinition, scan_mode: ScanMode) -> Self {
        let scene_patterns = compile_patterns(&definition.scene_file_patterns);
        let asset_patterns = compile_patterns(&definition.asset_file_patterns);
        let lock_patterns = compile_patterns(&definition.lock_required_patterns);
        let ignored_patterns = compile_patterns(&definition.ignored_patterns);
        let generated_patterns = compile_patterns(&definition.generated_cache_patterns);
        Self {
            definition,
            scan_mode,
            scene_patterns,
            asset_patterns,
            lock_patterns,
            ignored_patterns,
            generated_patterns,
        }
    }

    fn matches_file_type(&self, path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        self.definition
            .supported_file_types
            .iter()
            .any(|ext| lower.ends_with(&ext.to_ascii_lowercase()))
    }

    fn category_for(&self, path: &str) -> &'static str {
        if self.scene_patterns.is_match(path) {
            "scene"
        } else if self.asset_patterns.is_match(path) {
            "asset"
        } else {
            "unknown"
        }
    }

    fn has_lock_rule(&self, path: &str) -> bool {
        self.lock_patterns.is_match(path)
    }

    fn is_ignored(&self, path: &str) -> bool {
        self.ignored_patterns.is_match(path)
    }

    fn is_generated(&self, path: &str) -> bool {
        self.generated_patterns.is_match(path)
    }
}

impl DccAdapter for RuleBasedAdapter {
    fn definition(&self) -> &AdapterDefinition {
        &self.definition
    }

    fn detect_project(&self, probe: &ProjectProbe) -> Option<ProjectDetection> {
        for rule in &self.definition.project_detection_rules {
            if probe
                .relative_paths
                .iter()
                .any(|path| project_rule_matches(rule, path))
            {
                return Some(ProjectDetection {
                    app_name: self.definition.app_name.clone(),
                    root_path: probe.root_path.clone(),
                    matched_rule: rule.clone(),
                    confidence: if rule.contains('*') { 0.8 } else { 0.95 },
                });
            }
        }
        None
    }

    fn detect_open_file(&self, path: &str) -> bool {
        let Ok(path) = normalize_depot_path(path) else {
            return false;
        };
        self.matches_file_type(&path) || self.scene_patterns.is_match(&path)
    }

    fn scan_dependencies(&self, file: &AdapterFileInput) -> DependencyScanResult {
        let source = normalized_or_original(&file.path);
        let dependencies = match self.scan_mode {
            ScanMode::None => Vec::new(),
            ScanMode::UnityMeta => unity_meta_dependencies(&source),
            ScanMode::MayaAscii => maya_dependencies(&source, file.content.as_deref()),
            ScanMode::NukeScript => nuke_dependencies(&source, file.content.as_deref()),
        };
        DependencyScanResult {
            source_file: source,
            adapter_name: self.definition.app_name.clone(),
            dependencies,
        }
    }

    fn generate_preview(&self, file: &AdapterFileInput) -> PreviewGeneration {
        PreviewGeneration {
            file: normalized_or_original(&file.path),
            adapter_name: self.definition.app_name.clone(),
            strategy: self.definition.preview_generation_strategy.clone(),
            status: PreviewStatus::ExternalToolRequired,
            message: "MVP records preview strategy only; native preview generation is handled by future app plugins".to_string(),
        }
    }

    fn validate_before_submit(
        &self,
        changelist: &AdapterChangelist,
    ) -> Vec<AdapterValidationMessage> {
        let paths: HashSet<String> = changelist
            .files
            .iter()
            .map(|file| normalized_or_original(&file.path))
            .collect();
        let mut messages = Vec::new();

        for file in &changelist.files {
            let path = normalized_or_original(&file.path);
            if !self.detect_open_file(&path) && !self.is_generated(&path) && !self.is_ignored(&path)
            {
                continue;
            }

            if self.is_ignored(&path) {
                messages.push(self.warning(
                    &path,
                    "ignored_path",
                    "path matches an adapter ignore rule and should usually stay out of changelists",
                ));
            }
            if self.is_generated(&path) {
                messages.push(self.warning(
                    &path,
                    "generated_cache",
                    "path matches an adapter generated-cache rule and should usually be ignored",
                ));
            }
            if self.has_lock_rule(&path) {
                messages.push(self.warning(
                    &path,
                    "lock_required",
                    "file type normally requires an exclusive lock before submit",
                ));
            }
            if self.definition.app_name == "Unreal Engine" && path.ends_with(".ini") {
                messages.push(self.warning(
                    &path,
                    "unreal_config_changed",
                    "config changes can affect editor, cook, nDisplay, or Switchboard behavior; review before submit",
                ));
            }
            if self.definition.app_name == "Unity" && is_unity_asset_needing_meta(&path) {
                let meta = format!("{path}.meta");
                if !paths.contains(&meta) {
                    messages.push(self.warning(
                        &path,
                        "unity_meta_missing",
                        &format!("Unity metadata file is not in the changelist: {meta}"),
                    ));
                }
            }
            if self.definition.app_name == "Blender" && path.ends_with(".blend1") {
                messages.push(self.warning(
                    &path,
                    "blender_backup_file",
                    "Blender backup files are usually local safety copies; submit the .blend file instead unless intentional",
                ));
            }
            if self.definition.app_name == "Houdini"
                && (path.ends_with(".bgeo") || path.ends_with(".vdb"))
                && file.size_bytes.unwrap_or_default() >= 1_000_000_000
            {
                messages.push(self.warning(
                    &path,
                    "huge_houdini_cache",
                    "large Houdini cache files should be reviewed before adding to permanent depot history",
                ));
            }
            if self.definition.app_name == "Nuke" {
                for dependency in nuke_dependencies(&path, file.content.as_deref()) {
                    if dependency.dependency_status == DependencyStatus::External {
                        messages.push(self.warning(
                            &path,
                            "nuke_external_path",
                            "Nuke script references media outside the depot; relink or intentionally document the exception",
                        ));
                    }
                }
            }
        }

        dedupe_validation_messages(messages)
    }

    fn extract_metadata(&self, file: &AdapterFileInput) -> ExtractedMetadata {
        let path = normalized_or_original(&file.path);
        let mut metadata = BTreeMap::new();
        metadata.insert("category".to_string(), self.category_for(&path).to_string());
        metadata.insert(
            "extension".to_string(),
            extension_of(&path).unwrap_or_default(),
        );
        metadata.insert(
            "lock_required".to_string(),
            self.has_lock_rule(&path).to_string(),
        );
        metadata.insert(
            "generated".to_string(),
            self.is_generated(&path).to_string(),
        );
        metadata.insert("ignored".to_string(), self.is_ignored(&path).to_string());
        if let Some(size) = file.size_bytes {
            metadata.insert("size_bytes".to_string(), size.to_string());
        }
        ExtractedMetadata {
            file: path,
            adapter_name: self.definition.app_name.clone(),
            metadata,
        }
    }
}

impl RuleBasedAdapter {
    fn warning(&self, path: &str, code: &str, message: &str) -> AdapterValidationMessage {
        AdapterValidationMessage {
            path: path.to_string(),
            code: code.to_string(),
            severity: ValidationSeverity::Warning,
            message: message.to_string(),
            adapter_name: self.definition.app_name.clone(),
        }
    }
}

pub async fn list_adapters(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AdapterDefinition>>> {
    let _user = state.require_user(&headers)?;
    Ok(Json(state.adapters.definitions()))
}

pub async fn detect_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DetectProjectRequest>,
) -> AppResult<Json<Vec<ProjectDetection>>> {
    let _user = state.require_user(&headers)?;
    let probe = ProjectProbe {
        root_path: req.root_path,
        relative_paths: req
            .relative_paths
            .into_iter()
            .filter_map(|path| normalize_depot_path(&path).ok())
            .collect(),
    };
    Ok(Json(state.adapters.detect_project(&probe)))
}

pub async fn scan_dependencies(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AdapterScanRequest>,
) -> AppResult<Json<DependencyScanResult>> {
    let user = state.require_user(&headers)?;
    let result = state
        .adapters
        .scan_dependencies(req.adapter_name.as_deref(), &req.file)?;
    if let Some(workspace_id) = req.workspace_id {
        let workspace = workspace_for_user(&state.db, workspace_id, &user).await?;
        persist_dependency_scan(&state, &user, &workspace, &result).await?;
    }
    Ok(Json(result))
}

async fn persist_dependency_scan(
    state: &AppState,
    user: &AuthUser,
    workspace: &WorkspaceContext,
    result: &DependencyScanResult,
) -> AppResult<()> {
    let source_file = normalize_depot_path(&result.source_file)?;
    let mut normalized = Vec::with_capacity(result.dependencies.len());
    for dependency in &result.dependencies {
        let target_file = if dependency.dependency_status == DependencyStatus::External {
            validate_external_reference(&dependency.target_file)?
        } else {
            normalize_depot_path(&dependency.target_file)?
        };
        normalized.push((dependency, target_file));
    }

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "DELETE FROM asset_dependency_edges WHERE stream_id = $1 AND source_file = $2 AND adapter_name = $3",
    )
    .bind(workspace.stream_id)
    .bind(&source_file)
    .bind(&result.adapter_name)
    .execute(&mut *tx)
    .await?;

    for (dependency, target_file) in normalized {
        sqlx::query(
            r#"
            INSERT INTO asset_dependency_edges
                (stream_id, source_file, target_file, adapter_name,
                 dependency_type, dependency_status, confidence, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, '{}'::jsonb)
            "#,
        )
        .bind(workspace.stream_id)
        .bind(&source_file)
        .bind(target_file)
        .bind(&dependency.adapter_name)
        .bind(dependency.dependency_type.as_db_str())
        .bind(dependency.dependency_status.as_db_str())
        .bind(f64::from(dependency.confidence.clamp(0.0, 1.0)))
        .execute(&mut *tx)
        .await?;
    }

    audit::record_tx(
        &mut tx,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(workspace.stream_id),
            workspace_id: Some(workspace.id),
            depot_path: Some(&source_file),
            ..audit::AuditEvent::new(
                "dependency_scan",
                serde_json::json!({
                    "adapter": result.adapter_name,
                    "dependency_count": result.dependencies.len(),
                }),
            )
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

fn validate_external_reference(path: &str) -> AppResult<String> {
    let normalized = path.replace('\\', "/");
    if normalized.is_empty() || normalized.len() > 4_096 || normalized.chars().any(char::is_control)
    {
        return Err(AppError::bad_request("invalid external dependency path"));
    }
    Ok(normalized)
}

pub async fn generate_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AdapterScanRequest>,
) -> AppResult<Json<PreviewGeneration>> {
    let _user = state.require_user(&headers)?;
    Ok(Json(state.adapters.generate_preview(
        req.adapter_name.as_deref(),
        &req.file,
    )?))
}

pub async fn extract_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AdapterScanRequest>,
) -> AppResult<Json<ExtractedMetadata>> {
    let _user = state.require_user(&headers)?;
    Ok(Json(state.adapters.extract_metadata(
        req.adapter_name.as_deref(),
        &req.file,
    )?))
}

pub async fn validate_adapter_changelist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AdapterValidateRequest>,
) -> AppResult<Json<AdapterValidationResponse>> {
    let _user = state.require_user(&headers)?;
    let changelist = AdapterChangelist { files: req.files };
    let messages = state
        .adapters
        .validate_changelist(req.adapter_name.as_deref(), &changelist);
    Ok(Json(split_adapter_messages(messages)))
}

fn split_adapter_messages(messages: Vec<AdapterValidationMessage>) -> AdapterValidationResponse {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    for message in messages {
        match message.severity {
            ValidationSeverity::Warning => warnings.push(message),
            ValidationSeverity::Error => errors.push(message),
        }
    }
    AdapterValidationResponse { warnings, errors }
}

fn compile_patterns(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            Glob::new(pattern)
                .unwrap_or_else(|err| panic!("invalid built-in adapter glob {pattern}: {err}")),
        );
    }
    builder.build().expect("adapter glob set must compile")
}

fn project_rule_matches(rule: &str, path: &str) -> bool {
    let normalized = normalized_or_original(path);
    if rule.contains('*') {
        Glob::new(rule)
            .map(|glob| glob.compile_matcher().is_match(&normalized))
            .unwrap_or(false)
    } else {
        normalized == rule || normalized.ends_with(&format!("/{rule}"))
    }
}

fn normalized_or_original(path: &str) -> String {
    normalize_depot_path(path).unwrap_or_else(|_| path.replace('\\', "/"))
}

fn extension_of(path: &str) -> Option<String> {
    let file_name = path.rsplit('/').next()?;
    let index = file_name.rfind('.')?;
    Some(file_name[index..].to_ascii_lowercase())
}

pub fn is_unity_asset_needing_meta(path: &str) -> bool {
    path.starts_with("Assets/")
        && !path.ends_with(".meta")
        && (path.ends_with(".unity")
            || path.ends_with(".prefab")
            || path.ends_with(".asset")
            || path.ends_with(".mat")
            || path.ends_with(".controller")
            || path.ends_with(".anim")
            || path.ends_with(".shader")
            || path.ends_with(".cs")
            || path.ends_with(".fbx")
            || path.ends_with(".png")
            || path.ends_with(".exr")
            || path.ends_with(".wav"))
}

fn unity_meta_dependencies(source: &str) -> Vec<AssetDependency> {
    if !is_unity_asset_needing_meta(source) {
        return Vec::new();
    }
    vec![AssetDependency {
        source_file: source.to_string(),
        target_file: format!("{source}.meta"),
        dependency_type: DependencyType::Metadata,
        dependency_status: DependencyStatus::Unknown,
        adapter_name: "Unity".to_string(),
        confidence: 0.95,
    }]
}

fn maya_dependencies(source: &str, content: Option<&str>) -> Vec<AssetDependency> {
    let Some(content) = content else {
        return Vec::new();
    };
    MAYA_REFERENCE_RE
        .captures_iter(content)
        .filter_map(|captures| captures.get(1).map(|matched| matched.as_str()))
        .map(|target| text_dependency(source, target, "Maya", DependencyType::Reference, 0.72))
        .collect()
}

fn nuke_dependencies(source: &str, content: Option<&str>) -> Vec<AssetDependency> {
    let Some(content) = content else {
        return Vec::new();
    };
    NUKE_PATH_RE
        .captures_iter(content)
        .filter_map(|captures| captures.get(1).map(|matched| matched.as_str()))
        .map(|target| {
            let dependency_type = if target.ends_with(".cube") || target.ends_with(".lut") {
                DependencyType::Lut
            } else if target.ends_with(".mov") {
                DependencyType::Media
            } else {
                DependencyType::Plate
            };
            text_dependency(source, target, "Nuke", dependency_type, 0.7)
        })
        .collect()
}

fn text_dependency(
    source: &str,
    target: &str,
    adapter_name: &str,
    dependency_type: DependencyType,
    confidence: f32,
) -> AssetDependency {
    let target = target.trim_matches('"').replace('\\', "/");
    let dependency_status = if target.starts_with('/')
        || target.contains(":/")
        || target.starts_with("//")
        || target.starts_with("$")
    {
        DependencyStatus::External
    } else {
        DependencyStatus::Unknown
    };
    AssetDependency {
        source_file: source.to_string(),
        target_file: target,
        dependency_type,
        dependency_status,
        adapter_name: adapter_name.to_string(),
        confidence,
    }
}

fn dedupe_validation_messages(
    messages: Vec<AdapterValidationMessage>,
) -> Vec<AdapterValidationMessage> {
    let mut seen = HashSet::new();
    messages
        .into_iter()
        .filter(|message| {
            seen.insert((
                message.path.clone(),
                message.code.clone(),
                message.adapter_name.clone(),
            ))
        })
        .collect()
}

fn patterns_for_extensions(extensions: &[&str]) -> Vec<String> {
    extensions
        .iter()
        .flat_map(|extension| {
            [
                format!("*{}", extension.to_ascii_lowercase()),
                format!("**/*{}", extension.to_ascii_lowercase()),
            ]
        })
        .collect()
}

struct AdapterDefinitionSpec<'a> {
    app_name: &'a str,
    supported_file_types: &'a [&'a str],
    project_detection_rules: &'a [&'a str],
    scene_extensions: &'a [&'a str],
    asset_extensions: &'a [&'a str],
    lock_extensions: &'a [&'a str],
    ignored_patterns: &'a [&'a str],
    generated_cache_patterns: &'a [&'a str],
    dependency_scan_strategy: &'a str,
    preview_generation_strategy: &'a str,
    validation_rules: &'a [&'a str],
    metadata_extraction_rules: &'a [&'a str],
}

fn definition(spec: AdapterDefinitionSpec<'_>) -> AdapterDefinition {
    AdapterDefinition {
        app_name: spec.app_name.to_string(),
        supported_file_types: spec
            .supported_file_types
            .iter()
            .map(|value| value.to_string())
            .collect(),
        project_detection_rules: spec
            .project_detection_rules
            .iter()
            .map(|value| value.to_string())
            .collect(),
        scene_file_patterns: patterns_for_extensions(spec.scene_extensions),
        asset_file_patterns: patterns_for_extensions(spec.asset_extensions),
        lock_required_patterns: patterns_for_extensions(spec.lock_extensions),
        ignored_patterns: spec
            .ignored_patterns
            .iter()
            .map(|value| value.to_string())
            .collect(),
        generated_cache_patterns: spec
            .generated_cache_patterns
            .iter()
            .map(|value| value.to_string())
            .collect(),
        dependency_scan_strategy: spec.dependency_scan_strategy.to_string(),
        preview_generation_strategy: spec.preview_generation_strategy.to_string(),
        validation_rules: spec
            .validation_rules
            .iter()
            .map(|value| value.to_string())
            .collect(),
        metadata_extraction_rules: spec
            .metadata_extraction_rules
            .iter()
            .map(|value| value.to_string())
            .collect(),
    }
}

fn unreal_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Unreal Engine",
        supported_file_types: &[
            ".uasset",
            ".umap",
            ".uproject",
            ".uplugin",
            ".ini",
            ".usd",
            ".fbx",
            ".exr",
            ".wav",
            ".mov",
        ],
        project_detection_rules: &["*.uproject", "**/*.uproject"],
        scene_extensions: &[".umap"],
        asset_extensions: &[
            ".uasset", ".uplugin", ".usd", ".fbx", ".exr", ".wav", ".mov",
        ],
        lock_extensions: &[".uasset", ".umap"],
        ignored_patterns: &["Saved/**", "**/Saved/**"],
        generated_cache_patterns: &[
            "DerivedDataCache/**",
            "**/DerivedDataCache/**",
            "Intermediate/**",
            "**/Intermediate/**",
        ],
        dependency_scan_strategy: "future_asset_registry_and_commandlet_scanning",
        preview_generation_strategy: "future_editor_thumbnail_or_commandlet_preview",
        validation_rules: &[
            ".uasset and .umap require lock",
            "warn on .ini config changes",
            "future Asset Registry scanning",
            "future commandlet validation",
            "future Multi-User Editing baseline",
            "future nDisplay/Switchboard config tracking",
        ],
        metadata_extraction_rules: &[
            "extension",
            "lock_required",
            "generated_cache",
            "future_asset_registry",
        ],
    })
}

fn unity_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Unity",
        supported_file_types: &[
            ".unity",
            ".prefab",
            ".mat",
            ".asset",
            ".controller",
            ".anim",
            ".meta",
            ".cs",
            ".shader",
            ".fbx",
            ".png",
            ".exr",
            ".wav",
        ],
        project_detection_rules: &[
            "ProjectSettings/ProjectVersion.txt",
            "Packages/manifest.json",
        ],
        scene_extensions: &[".unity"],
        asset_extensions: &[
            ".prefab",
            ".mat",
            ".asset",
            ".controller",
            ".anim",
            ".meta",
            ".cs",
            ".shader",
            ".fbx",
            ".png",
            ".exr",
            ".wav",
        ],
        lock_extensions: &[".unity", ".prefab"],
        ignored_patterns: &["Logs/**", "**/Logs/**"],
        generated_cache_patterns: &["Library/**", "**/Library/**", "Temp/**", "**/Temp/**"],
        dependency_scan_strategy: "rule_based_meta_pair_detection_future_guid_graph",
        preview_generation_strategy: "future_unity_batchmode_or_asset_preview",
        validation_rules: &[
            ".unity and .prefab require lock",
            ".meta files must be tracked",
            "warn when asset file changed without matching .meta",
            "future GUID dependency graph",
            "future UnityYAMLMerge support",
        ],
        metadata_extraction_rules: &["extension", "unity_meta_pair", "future_guid"],
    })
}

fn blender_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Blender",
        supported_file_types: &[
            ".blend", ".blend1", ".fbx", ".obj", ".usd", ".abc", ".exr", ".png", ".hdr", ".wav",
        ],
        project_detection_rules: &["*.blend", "**/*.blend"],
        scene_extensions: &[".blend"],
        asset_extensions: &[
            ".fbx", ".obj", ".usd", ".abc", ".exr", ".png", ".hdr", ".wav",
        ],
        lock_extensions: &[".blend"],
        ignored_patterns: &["*.blend1", "**/*.blend1"],
        generated_cache_patterns: &[],
        dependency_scan_strategy: "future_blender_python_linked_library_scan",
        preview_generation_strategy: "future_blender_python_preview",
        validation_rules: &[
            ".blend requires lock",
            "warn about .blend1 backup files",
            "future linked library detection",
            "future packed/external asset detection",
        ],
        metadata_extraction_rules: &["extension", "backup_detection", "future_linked_libraries"],
    })
}

fn maya_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Maya",
        supported_file_types: &[
            ".ma", ".mb", ".fbx", ".abc", ".usd", ".obj", ".exr", ".tif", ".tx", ".wav",
        ],
        project_detection_rules: &["workspace.mel", "*.ma", "**/*.ma", "*.mb", "**/*.mb"],
        scene_extensions: &[".ma", ".mb"],
        asset_extensions: &[
            ".fbx", ".abc", ".usd", ".obj", ".exr", ".tif", ".tx", ".wav",
        ],
        lock_extensions: &[".ma", ".mb"],
        ignored_patterns: &[],
        generated_cache_patterns: &[],
        dependency_scan_strategy: "simple_maya_ascii_reference_regex_future_maya_python",
        preview_generation_strategy: "future_maya_viewport_preview",
        validation_rules: &[
            ".ma and .mb require lock",
            ".ma can be text scanned for file references",
            "future Maya Python reference scanning",
            "future scene FPS/unit validation",
        ],
        metadata_extraction_rules: &["extension", "maya_ascii_references"],
    })
}

fn houdini_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Houdini",
        supported_file_types: &[
            ".hip", ".hipnc", ".hda", ".usd", ".abc", ".bgeo", ".vdb", ".exr",
        ],
        project_detection_rules: &["*.hip", "**/*.hip", "*.hipnc", "**/*.hipnc"],
        scene_extensions: &[".hip", ".hipnc"],
        asset_extensions: &[".hda", ".usd", ".abc", ".bgeo", ".vdb", ".exr"],
        lock_extensions: &[".hip", ".hda"],
        ignored_patterns: &[],
        generated_cache_patterns: &["*.bgeo", "**/*.bgeo", "*.vdb", "**/*.vdb"],
        dependency_scan_strategy: "future_houdini_python_node_scan",
        preview_generation_strategy: "future_houdini_viewport_preview",
        validation_rules: &[
            ".hip and .hda require lock",
            ".bgeo and .vdb are generated cache by default",
            "warn when huge cache files are added",
        ],
        metadata_extraction_rules: &["extension", "generated_cache"],
    })
}

fn nuke_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Nuke",
        supported_file_types: &[".nk", ".exr", ".dpx", ".mov", ".cube", ".lut"],
        project_detection_rules: &["*.nk", "**/*.nk"],
        scene_extensions: &[".nk"],
        asset_extensions: &[".exr", ".dpx", ".mov", ".cube", ".lut"],
        lock_extensions: &[],
        ignored_patterns: &[],
        generated_cache_patterns: &[],
        dependency_scan_strategy: "simple_read_write_path_regex_future_node_graph",
        preview_generation_strategy: "future_nuke_frame_or_proxy_preview",
        validation_rules: &[
            ".nk can be text scanned",
            ".nk may require lock by site policy",
            "future Read/Write dependency extraction",
            "warn if plate/LUT paths are outside depot",
        ],
        metadata_extraction_rules: &["extension", "nuke_read_write_paths"],
    })
}

fn premiere_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Adobe Premiere",
        supported_file_types: &[".prproj", ".mov", ".wav", ".exr", ".png", ".jpg", ".cube"],
        project_detection_rules: &["*.prproj", "**/*.prproj"],
        scene_extensions: &[".prproj"],
        asset_extensions: &[".mov", ".wav", ".exr", ".png", ".jpg", ".cube"],
        lock_extensions: &[".prproj"],
        ignored_patterns: &[
            "Adobe Premiere Pro Auto-Save/**",
            "**/Adobe Premiere Pro Auto-Save/**",
        ],
        generated_cache_patterns: &[],
        dependency_scan_strategy: "future_linked_media_detection",
        preview_generation_strategy: "future_proxy_preview_generation",
        validation_rules: &[
            ".prproj requires lock",
            "future linked media detection",
            "future proxy preview generation",
            "future timecode review",
        ],
        metadata_extraction_rules: &["extension", "future_linked_media"],
    })
}

fn after_effects_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "After Effects",
        supported_file_types: &[
            ".aep", ".mov", ".wav", ".exr", ".png", ".jpg", ".psd", ".cube",
        ],
        project_detection_rules: &["*.aep", "**/*.aep"],
        scene_extensions: &[".aep"],
        asset_extensions: &[".mov", ".wav", ".exr", ".png", ".jpg", ".psd", ".cube"],
        lock_extensions: &[".aep"],
        ignored_patterns: &[
            "Adobe After Effects Auto-Save/**",
            "**/Adobe After Effects Auto-Save/**",
        ],
        generated_cache_patterns: &[],
        dependency_scan_strategy: "future_linked_media_detection",
        preview_generation_strategy: "future_proxy_preview_generation",
        validation_rules: &[
            ".aep requires lock",
            "future linked media detection",
            "future proxy preview generation",
            "future timecode review",
        ],
        metadata_extraction_rules: &["extension", "future_linked_media"],
    })
}

fn photoshop_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Photoshop",
        supported_file_types: &[".psd", ".psb", ".png", ".jpg", ".exr", ".tif"],
        project_detection_rules: &["*.psd", "**/*.psd", "*.psb", "**/*.psb"],
        scene_extensions: &[],
        asset_extensions: &[".psd", ".psb", ".png", ".jpg", ".exr", ".tif"],
        lock_extensions: &[".psd", ".psb"],
        ignored_patterns: &[],
        generated_cache_patterns: &[],
        dependency_scan_strategy: "future_linked_smart_object_detection",
        preview_generation_strategy: "future_layer_or_proxy_preview",
        validation_rules: &[
            ".psd and .psb require lock",
            "future linked media detection",
            "future proxy preview generation",
        ],
        metadata_extraction_rules: &["extension", "lock_required"],
    })
}

fn resolve_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "DaVinci Resolve",
        supported_file_types: &[".drp", ".mov", ".wav", ".exr", ".png", ".jpg", ".cube"],
        project_detection_rules: &["*.drp", "**/*.drp"],
        scene_extensions: &[".drp"],
        asset_extensions: &[".mov", ".wav", ".exr", ".png", ".jpg", ".cube"],
        lock_extensions: &[".drp"],
        ignored_patterns: &[],
        generated_cache_patterns: &[
            "CacheClip/**",
            "**/CacheClip/**",
            "OptimizedMedia/**",
            "**/OptimizedMedia/**",
        ],
        dependency_scan_strategy: "future_resolve_media_pool_detection",
        preview_generation_strategy: "future_proxy_preview_generation",
        validation_rules: &[
            ".drp requires lock",
            "future linked media detection",
            "future proxy preview generation",
            "future timecode review",
        ],
        metadata_extraction_rules: &["extension", "future_media_pool"],
    })
}

fn generic_media_definition() -> AdapterDefinition {
    definition(AdapterDefinitionSpec {
        app_name: "Generic Media",
        supported_file_types: &[
            ".mov", ".mp4", ".wav", ".exr", ".png", ".jpg", ".jpeg", ".tif", ".tiff", ".usd",
            ".fbx", ".abc",
        ],
        project_detection_rules: &[],
        scene_extensions: &[],
        asset_extensions: &[
            ".mov", ".mp4", ".wav", ".exr", ".png", ".jpg", ".jpeg", ".tif", ".tiff", ".usd",
            ".fbx", ".abc",
        ],
        lock_extensions: &[],
        ignored_patterns: &[],
        generated_cache_patterns: &["Cache/**", "**/Cache/**", "cache/**", "**/cache/**"],
        dependency_scan_strategy: "none",
        preview_generation_strategy: "future_ffmpeg_or_image_proxy_preview",
        validation_rules: &[
            "large media files stream and chunk",
            "future proxy preview generation",
        ],
        metadata_extraction_rules: &["extension", "size"],
    })
}

#[cfg(test)]
mod tests {
    use super::{
        is_unity_asset_needing_meta, AdapterChangelist, AdapterFileInput, AdapterRegistry,
        DependencyStatus, ProjectProbe,
    };

    #[test]
    fn registry_contains_required_adapters() {
        let registry = AdapterRegistry::default_registry();
        let names: Vec<_> = registry
            .definitions()
            .into_iter()
            .map(|definition| definition.app_name)
            .collect();
        for required in [
            "Unreal Engine",
            "Unity",
            "Blender",
            "Maya",
            "Houdini",
            "Nuke",
            "Adobe Premiere",
            "After Effects",
            "Photoshop",
            "DaVinci Resolve",
            "Generic Media",
        ] {
            assert!(names.iter().any(|name| name == required), "{required}");
        }
    }

    #[test]
    fn detects_unreal_and_unity_projects() {
        let registry = AdapterRegistry::default_registry();
        let unreal = registry.detect_project(&ProjectProbe {
            root_path: "/show/game".to_string(),
            relative_paths: vec!["Shooter.uproject".to_string()],
        });
        assert_eq!(unreal[0].app_name, "Unreal Engine");

        let unity = registry.detect_project(&ProjectProbe {
            root_path: "/show/unity".to_string(),
            relative_paths: vec!["ProjectSettings/ProjectVersion.txt".to_string()],
        });
        assert_eq!(unity[0].app_name, "Unity");
    }

    #[test]
    fn unity_validation_warns_about_missing_meta() {
        let registry = AdapterRegistry::default_registry();
        let messages = registry.validate_changelist(
            Some("Unity"),
            &AdapterChangelist {
                files: vec![AdapterFileInput {
                    path: "Assets/Scenes/Main.unity".to_string(),
                    content: None,
                    size_bytes: None,
                }],
            },
        );
        assert!(messages
            .iter()
            .any(|message| message.code == "unity_meta_missing"));
        assert!(is_unity_asset_needing_meta("Assets/Scenes/Main.unity"));
    }

    #[test]
    fn maya_ascii_scanner_finds_text_references() {
        let registry = AdapterRegistry::default_registry();
        let scan = registry
            .scan_dependencies(
                Some("Maya"),
                &AdapterFileInput {
                    path: "Shots/shot010.ma".to_string(),
                    content: Some(r#"file -r -ns "rig" "Assets/Rigs/Hero.ma";"#.to_string()),
                    size_bytes: Some(12),
                },
            )
            .unwrap();
        assert_eq!(scan.dependencies[0].target_file, "Assets/Rigs/Hero.ma");
    }

    #[test]
    fn nuke_scanner_marks_absolute_paths_external() {
        let registry = AdapterRegistry::default_registry();
        let scan = registry
            .scan_dependencies(
                Some("Nuke"),
                &AdapterFileInput {
                    path: "Comp/shot010.nk".to_string(),
                    content: Some("Read {\n file /Volumes/plates/shot010.exr\n}".to_string()),
                    size_bytes: None,
                },
            )
            .unwrap();
        assert_eq!(
            scan.dependencies[0].dependency_status,
            DependencyStatus::External
        );
    }
}
