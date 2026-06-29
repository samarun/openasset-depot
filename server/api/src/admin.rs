use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::AppState,
    audit,
    error::{AppError, AppResult},
    storage::BlobIntegrityReport,
};

#[derive(Debug, Deserialize)]
pub struct VerifyStorageQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct StorageVerificationSummary {
    pub checked: usize,
    pub ok: usize,
    pub failed: usize,
    pub failures: Vec<BlobIntegrityReport>,
}

#[derive(Debug, Deserialize)]
pub struct StorageCleanupRequest {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    pub older_than_hours: Option<u64>,
    pub max_delete: Option<usize>,
}

#[derive(Debug, Default, Serialize)]
pub struct StorageCleanupSummary {
    pub dry_run: bool,
    pub scanned: usize,
    pub referenced: usize,
    pub orphaned: usize,
    pub orphaned_bytes: u64,
    pub deleted: usize,
    pub deleted_bytes: u64,
    pub skipped_recent: usize,
    pub invalid_files: usize,
    pub delete_limit_reached: bool,
}

#[derive(Debug)]
struct ObjectCandidate {
    hash: String,
    path: PathBuf,
    size_bytes: u64,
}

pub async fn verify_storage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<VerifyStorageQuery>,
) -> AppResult<Json<StorageVerificationSummary>> {
    let user = state.require_user(&headers)?;
    if !user.is_admin {
        return Err(AppError::Forbidden(
            "only admins can verify object storage".to_string(),
        ));
    }
    let _storage_lease = state.storage_maintenance.read().await;
    let limit = query.limit.unwrap_or(100).clamp(1, 1_000);
    let hashes: Vec<String> =
        sqlx::query_scalar("SELECT hash FROM blobs ORDER BY created_at DESC LIMIT $1")
            .bind(limit)
            .fetch_all(&state.db)
            .await?;

    let mut ok = 0usize;
    let mut failures = Vec::new();
    for hash in hashes {
        let report = state.storage.verify_blob(&hash).await;
        if report.ok {
            ok += 1;
        } else {
            failures.push(report);
        }
    }
    let summary = StorageVerificationSummary {
        checked: ok + failures.len(),
        ok,
        failed: failures.len(),
        failures,
    };
    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            ..audit::AuditEvent::new(
                "storage_verify",
                serde_json::json!({
                    "checked": summary.checked,
                    "failed": summary.failed,
                }),
            )
        },
    )
    .await?;
    Ok(Json(summary))
}

pub async fn cleanup_storage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<StorageCleanupRequest>,
) -> AppResult<Json<StorageCleanupSummary>> {
    let user = state.require_user(&headers)?;
    if !user.is_admin {
        return Err(AppError::Forbidden(
            "only admins can clean object storage".to_string(),
        ));
    }
    let older_than_hours = request.older_than_hours.unwrap_or(24).clamp(1, 8_760);
    let max_delete = request.max_delete.unwrap_or(10_000).clamp(1, 100_000);
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(older_than_hours * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let _storage_lease = state.storage_maintenance.write().await;
    let mut summary = StorageCleanupSummary {
        dry_run: request.dry_run,
        ..StorageCleanupSummary::default()
    };

    scan_object_tree(
        &state,
        state.storage.blob_manifests_root(),
        true,
        cutoff,
        max_delete,
        &mut summary,
    )
    .await?;
    scan_object_tree(
        &state,
        state.storage.chunk_files_root(),
        false,
        cutoff,
        max_delete,
        &mut summary,
    )
    .await?;
    summary.delete_limit_reached = !request.dry_run && summary.deleted >= max_delete;

    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            ..audit::AuditEvent::new(
                "storage_cleanup",
                serde_json::json!({
                    "dry_run": summary.dry_run,
                    "scanned": summary.scanned,
                    "orphaned": summary.orphaned,
                    "deleted": summary.deleted,
                    "deleted_bytes": summary.deleted_bytes,
                    "older_than_hours": older_than_hours,
                }),
            )
        },
    )
    .await?;
    Ok(Json(summary))
}

async fn scan_object_tree(
    state: &AppState,
    root: PathBuf,
    manifests: bool,
    cutoff: SystemTime,
    max_delete: usize,
    summary: &mut StorageCleanupSummary,
) -> AppResult<()> {
    let mut batch = Vec::with_capacity(1_000);
    let mut first_level = tokio::fs::read_dir(root).await?;
    while let Some(first) = first_level.next_entry().await? {
        if !first.file_type().await?.is_dir() {
            summary.invalid_files += 1;
            continue;
        }
        let mut second_level = tokio::fs::read_dir(first.path()).await?;
        while let Some(second) = second_level.next_entry().await? {
            if !second.file_type().await?.is_dir() {
                summary.invalid_files += 1;
                continue;
            }
            let mut objects = tokio::fs::read_dir(second.path()).await?;
            while let Some(object) = objects.next_entry().await? {
                let metadata = object.metadata().await?;
                if !metadata.is_file() {
                    summary.invalid_files += 1;
                    continue;
                }
                summary.scanned += 1;
                if metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH) > cutoff {
                    summary.skipped_recent += 1;
                    continue;
                }
                let name = object.file_name().to_string_lossy().into_owned();
                let hash = if manifests {
                    name.strip_suffix(".json").map(str::to_string)
                } else {
                    Some(name)
                };
                let Some(hash) = hash.filter(|hash| valid_hash(hash)) else {
                    summary.invalid_files += 1;
                    continue;
                };
                batch.push(ObjectCandidate {
                    hash,
                    path: object.path(),
                    size_bytes: metadata.len(),
                });
                if batch.len() == 1_000 {
                    process_object_batch(state, manifests, &mut batch, max_delete, summary).await?;
                }
            }
        }
    }
    process_object_batch(state, manifests, &mut batch, max_delete, summary).await
}

async fn process_object_batch(
    state: &AppState,
    manifests: bool,
    batch: &mut Vec<ObjectCandidate>,
    max_delete: usize,
    summary: &mut StorageCleanupSummary,
) -> AppResult<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let hashes: Vec<String> = batch.iter().map(|item| item.hash.clone()).collect();
    let rows: Vec<String> = if manifests {
        sqlx::query_scalar("SELECT hash FROM blobs WHERE hash = ANY($1)")
            .bind(&hashes)
            .fetch_all(&state.db)
            .await?
    } else {
        sqlx::query_scalar("SELECT hash FROM chunks WHERE hash = ANY($1)")
            .bind(&hashes)
            .fetch_all(&state.db)
            .await?
    };
    let referenced: HashSet<_> = rows.into_iter().collect();
    for candidate in batch.drain(..) {
        if referenced.contains(&candidate.hash) {
            summary.referenced += 1;
            continue;
        }
        summary.orphaned += 1;
        summary.orphaned_bytes += candidate.size_bytes;
        if !summary.dry_run && summary.deleted < max_delete {
            tokio::fs::remove_file(candidate.path).await?;
            summary.deleted += 1;
            summary.deleted_bytes += candidate.size_bytes;
        }
    }
    Ok(())
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::valid_hash;

    #[test]
    fn maintenance_hash_validation_is_strict() {
        assert!(valid_hash(&"a".repeat(64)));
        assert!(!valid_hash(&"a".repeat(63)));
        assert!(!valid_hash(&format!("{}z", "a".repeat(63))));
    }
}
