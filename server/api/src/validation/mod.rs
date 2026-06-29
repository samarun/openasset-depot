use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    adapters::{AdapterChangelist, AdapterFileInput, ValidationSeverity},
    api::AppState,
    audit,
    error::AppResult,
    paths::normalize_depot_path,
    workspaces::workspace_for_user,
};

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub workspace_id: Uuid,
    pub paths: Vec<ValidatePath>,
}

#[derive(Debug, Deserialize)]
pub struct ValidatePath {
    pub path: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ValidationResponse {
    pub warnings: Vec<ValidationMessage>,
    pub errors: Vec<ValidationMessage>,
}

#[derive(Debug, Serialize)]
pub struct ValidationMessage {
    pub path: String,
    pub code: String,
    pub message: String,
}

pub async fn validate_paths(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ValidateRequest>,
) -> AppResult<Json<ValidationResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let matcher = state.filetypes.read().await.clone();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let normalized: Vec<_> = req
        .paths
        .iter()
        .map(|path| {
            normalize_depot_path(&path.path).map(|normalized| (normalized, path.size_bytes))
        })
        .collect::<Result<_, _>>()?;
    let path_set: std::collections::HashSet<_> =
        normalized.iter().map(|(path, _)| path.clone()).collect();

    for (path, size_bytes) in normalized {
        let rule = matcher.match_path(&path)?;
        if rule.generated {
            warnings.push(ValidationMessage {
                path: path.clone(),
                code: "generated_cache".to_string(),
                message: "path matches a generated cache rule and should usually be ignored"
                    .to_string(),
            });
        }
        if rule.large_file || size_bytes.unwrap_or_default() >= 1_000_000_000 {
            warnings.push(ValidationMessage {
                path: path.clone(),
                code: "large_file".to_string(),
                message: "large file will be streamed and chunked during submit".to_string(),
            });
        }
        if is_unity_asset_needing_meta(&path) {
            let meta = format!("{path}.meta");
            if !path_set.contains(meta.as_str()) {
                warnings.push(ValidationMessage {
                    path: path.clone(),
                    code: "unity_meta_missing".to_string(),
                    message: format!("Unity metadata file is not in the validation set: {meta}"),
                });
            }
        }
    }

    let adapter_files = req
        .paths
        .iter()
        .map(|path| {
            normalize_depot_path(&path.path).map(|normalized| AdapterFileInput {
                path: normalized,
                content: None,
                size_bytes: path.size_bytes,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let adapter_messages = state.adapters.validate_changelist(
        None,
        &AdapterChangelist {
            files: adapter_files,
        },
    );
    for message in adapter_messages {
        let validation_message = ValidationMessage {
            path: message.path,
            code: message.code,
            message: format!("{}: {}", message.adapter_name, message.message),
        };
        match message.severity {
            ValidationSeverity::Warning => warnings.push(validation_message),
            ValidationSeverity::Error => errors.push(validation_message),
        }
    }

    dedupe_messages(&mut warnings);
    dedupe_messages(&mut errors);

    if !errors.is_empty() {
        audit::record(
            &state.db,
            audit::AuditEvent {
                actor_user_id: Some(user.user_id),
                stream_id: Some(workspace.stream_id),
                workspace_id: Some(workspace.id),
                ..audit::AuditEvent::new(
                    "validation_failed",
                    serde_json::json!({ "error_count": errors.len() }),
                )
            },
        )
        .await?;
    }

    Ok(Json(ValidationResponse { warnings, errors }))
}

pub fn is_unity_asset_needing_meta(path: &str) -> bool {
    crate::adapters::is_unity_asset_needing_meta(path)
}

fn dedupe_messages(messages: &mut Vec<ValidationMessage>) {
    let mut seen = std::collections::HashSet::new();
    messages.retain(|message| {
        seen.insert((
            message.path.clone(),
            message.code.clone(),
            message.message.clone(),
        ))
    });
}

#[cfg(test)]
mod tests {
    use super::is_unity_asset_needing_meta;

    #[test]
    fn detects_unity_meta_candidates() {
        assert!(is_unity_asset_needing_meta("Assets/Scene.unity"));
        assert!(is_unity_asset_needing_meta("Assets/Prefab.prefab"));
        assert!(!is_unity_asset_needing_meta("Assets/Scene.unity.meta"));
        assert!(!is_unity_asset_needing_meta("Content/Map.umap"));
    }
}
