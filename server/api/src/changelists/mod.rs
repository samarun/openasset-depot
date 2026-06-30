use std::collections::HashSet;

use axum::{
    extract::{Multipart, Path, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    auth::AuthUser,
    error::{AppError, AppResult},
    idempotency,
    locking::user_holds_lock,
    paths::normalize_depot_path,
    storage::BlobManifest,
    workspaces::{workspace_for_user, WorkspaceContext},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateChangelistRequest {
    pub workspace_id: Uuid,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelistResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub description: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct FileOpRequest {
    pub workspace_id: Uuid,
    pub changelist_id: Option<Uuid>,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct FileOpResponse {
    pub path: String,
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitResponse {
    pub changelist_id: Uuid,
    pub revisions: Vec<SubmittedRevision>,
}

#[derive(Debug, Serialize)]
pub struct SubmittedRevision {
    pub path: String,
    pub revision_number: i32,
    pub blob_hash: String,
    pub size_bytes: i64,
}

#[derive(Debug)]
struct SubmittedUpload {
    path: String,
    manifest: BlobManifest,
}

pub async fn create_changelist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateChangelistRequest>,
) -> AppResult<Json<ChangelistResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    if req.description.trim().is_empty() {
        return Err(AppError::bad_request("changelist description is required"));
    }
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "changelist_create",
        &req,
        || async {
            let row = sqlx::query(
                r#"
                INSERT INTO changelists (user_id, workspace_id, description)
                VALUES ($1, $2, $3)
                RETURNING id, user_id, workspace_id, description, status, created_at
                "#,
            )
            .bind(user.user_id)
            .bind(workspace.id)
            .bind(&req.description)
            .fetch_one(&state.db)
            .await?;
            let response = changelist_response(row);
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(workspace.stream_id),
                    workspace_id: Some(workspace.id),
                    changelist_id: Some(response.id),
                    ..audit::AuditEvent::new(
                        "changelist_create",
                        serde_json::json!({ "description": response.description }),
                    )
                },
            )
            .await?;
            Ok(response)
        },
    )
    .await?;
    Ok(Json(response))
}

pub async fn file_add(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FileOpRequest>,
) -> AppResult<Json<FileOpResponse>> {
    file_op(state, headers, req, "add", "file_add").await
}

pub async fn file_edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FileOpRequest>,
) -> AppResult<Json<FileOpResponse>> {
    file_op(state, headers, req, "edit", "file_edit").await
}

pub async fn file_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FileOpRequest>,
) -> AppResult<Json<FileOpResponse>> {
    file_op(state, headers, req, "delete", "file_delete").await
}

pub async fn file_revert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FileOpRequest>,
) -> AppResult<Json<FileOpResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let path = normalize_depot_path(&req.path)?;

    if let Some(changelist_id) = req.changelist_id {
        ensure_pending_changelist_for_workspace(
            &state.db,
            changelist_id,
            user.user_id,
            workspace.id,
        )
        .await?;
        sqlx::query("DELETE FROM changelist_files WHERE changelist_id = $1 AND depot_path = $2")
            .bind(changelist_id)
            .bind(&path)
            .execute(&state.db)
            .await?;
    }
    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(workspace.stream_id),
            workspace_id: Some(workspace.id),
            depot_path: Some(&path),
            changelist_id: req.changelist_id,
            ..audit::AuditEvent::new("revert", serde_json::json!({}))
        },
    )
    .await?;
    Ok(Json(FileOpResponse {
        path,
        action: "revert".to_string(),
    }))
}

async fn file_op(
    state: AppState,
    headers: HeaderMap,
    req: FileOpRequest,
    action: &'static str,
    audit_event: &'static str,
) -> AppResult<Json<FileOpResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let path = normalize_depot_path(&req.path)?;

    if action == "delete" {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM files WHERE stream_id = $1 AND depot_path = $2 AND deleted = FALSE)",
        )
        .bind(workspace.stream_id)
        .bind(&path)
        .fetch_one(&state.db)
        .await?;
        if !exists {
            return Err(AppError::NotFound(format!("file not found: {path}")));
        }
    }

    if let Some(changelist_id) = req.changelist_id {
        ensure_pending_changelist_for_workspace(
            &state.db,
            changelist_id,
            user.user_id,
            workspace.id,
        )
        .await?;
        sqlx::query(
            r#"
            INSERT INTO changelist_files (changelist_id, depot_path, action)
            VALUES ($1, $2, $3)
            ON CONFLICT (changelist_id, depot_path)
            DO UPDATE SET action = EXCLUDED.action
            "#,
        )
        .bind(changelist_id)
        .bind(&path)
        .bind(action)
        .execute(&state.db)
        .await?;
    }

    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(workspace.stream_id),
            workspace_id: Some(workspace.id),
            depot_path: Some(&path),
            changelist_id: req.changelist_id,
            ..audit::AuditEvent::new(audit_event, serde_json::json!({ "action": action }))
        },
    )
    .await?;

    Ok(Json(FileOpResponse {
        path,
        action: action.to_string(),
    }))
}

pub async fn submit_changelist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(changelist_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<SubmitResponse>> {
    let user = state.require_user(&headers)?;
    let _storage_lease = state.storage_maintenance.read().await;
    let mut workspace_id = None;
    let mut uploads = Vec::new();
    let mut submitted_paths = HashSet::new();

    while let Some(mut field) = multipart.next_field().await? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "workspace_id" {
            let value = field.text().await?;
            workspace_id = Some(
                Uuid::parse_str(value.trim())
                    .map_err(|_| AppError::bad_request("workspace_id must be a UUID"))?,
            );
            continue;
        }

        if name == "file" {
            let file_name = field
                .file_name()
                .ok_or_else(|| AppError::bad_request("file part is missing a file name"))?
                .to_string();
            let depot_path = normalize_depot_path(&file_name)?;
            if !submitted_paths.insert(depot_path.clone()) {
                return Err(AppError::bad_request(format!(
                    "duplicate file part for depot path {depot_path}"
                )));
            }
            let temp_path = state.storage.temp_upload_path(Uuid::new_v4());
            let result: AppResult<BlobManifest> = async {
                let mut out = tokio::fs::File::create(&temp_path).await?;
                while let Some(chunk) = field.chunk().await? {
                    out.write_all(&chunk).await?;
                }
                out.flush().await?;
                state.storage.put_file(&temp_path).await
            }
            .await;
            let _ = tokio::fs::remove_file(&temp_path).await;
            uploads.push(SubmittedUpload {
                path: depot_path,
                manifest: result?,
            });
        }
    }

    let workspace_id = workspace_id
        .ok_or_else(|| AppError::bad_request("multipart field workspace_id is required"))?;
    let workspace = workspace_for_user(&state.db, workspace_id, &user).await?;
    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(workspace.stream_id),
            workspace_id: Some(workspace.id),
            changelist_id: Some(changelist_id),
            ..audit::AuditEvent::new(
                "submit_start",
                serde_json::json!({ "upload_count": uploads.len() }),
            )
        },
    )
    .await?;

    let result = submit_uploads(&state, &user, &workspace, changelist_id, uploads).await;
    match result {
        Ok(revisions) => {
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(workspace.stream_id),
                    workspace_id: Some(workspace.id),
                    changelist_id: Some(changelist_id),
                    ..audit::AuditEvent::new(
                        "submit_success",
                        serde_json::json!({ "file_count": revisions.len() }),
                    )
                },
            )
            .await?;
            Ok(Json(SubmitResponse {
                changelist_id,
                revisions,
            }))
        }
        Err(error) => {
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(workspace.stream_id),
                    workspace_id: Some(workspace.id),
                    changelist_id: Some(changelist_id),
                    ..audit::AuditEvent::new(
                        "submit_failed",
                        serde_json::json!({ "message": error.to_string() }),
                    )
                },
            )
            .await?;
            Err(error)
        }
    }
}

async fn submit_uploads(
    state: &AppState,
    user: &AuthUser,
    workspace: &WorkspaceContext,
    changelist_id: Uuid,
    uploads: Vec<SubmittedUpload>,
) -> AppResult<Vec<SubmittedRevision>> {
    let matcher = state.filetypes.read().await.clone();
    let mut tx = state.db.begin().await?;

    let changelist = sqlx::query(
        r#"
        SELECT id, status
        FROM changelists
        WHERE id = $1 AND user_id = $2 AND workspace_id = $3
        FOR UPDATE
        "#,
    )
    .bind(changelist_id)
    .bind(user.user_id)
    .bind(workspace.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("changelist not found".to_string()))?;
    let status: String = changelist.get("status");
    if status == "submitted" {
        return submitted_retry_result(&mut tx, changelist_id, &uploads).await;
    }
    if status != "pending" {
        return Err(AppError::conflict(
            "only pending changelists can be submitted",
        ));
    }

    let delete_paths: Vec<String> = sqlx::query_scalar(
        "SELECT depot_path FROM changelist_files WHERE changelist_id = $1 AND action = 'delete' ORDER BY depot_path",
    )
    .bind(changelist_id)
    .fetch_all(&mut *tx)
    .await?;
    let content_paths: Vec<String> = sqlx::query_scalar(
        "SELECT depot_path FROM changelist_files WHERE changelist_id = $1 AND action IN ('add', 'edit') ORDER BY depot_path",
    )
    .bind(changelist_id)
    .fetch_all(&mut *tx)
    .await?;
    if uploads.is_empty() && delete_paths.is_empty() {
        return Err(AppError::bad_request(
            "submit requires at least one added, edited, or deleted file",
        ));
    }
    let uploaded_paths: HashSet<_> = uploads.iter().map(|upload| upload.path.as_str()).collect();
    for path in &content_paths {
        if !uploaded_paths.contains(path.as_str()) {
            return Err(AppError::bad_request(format!(
                "submit is missing file content for {path}"
            )));
        }
    }
    for path in &delete_paths {
        if uploaded_paths.contains(path.as_str()) {
            return Err(AppError::bad_request(format!(
                "deleted path {path} must not include file content"
            )));
        }
    }

    let mut revisions = Vec::new();
    for upload in uploads {
        let rule = matcher.match_path(&upload.path)?;
        if rule.lock_required
            && !user_holds_lock(
                &mut tx,
                workspace.stream_id,
                workspace.id,
                user.user_id,
                &upload.path,
            )
            .await?
        {
            return Err(AppError::conflict(format!(
                "{} requires an active exclusive lock held by the submitting user",
                upload.path
            )));
        }

        insert_blob_records(&mut tx, &upload.manifest).await?;

        let file_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO files (stream_id, depot_path)
            VALUES ($1, $2)
            ON CONFLICT (stream_id, depot_path)
            DO UPDATE SET updated_at = now()
            RETURNING id
            "#,
        )
        .bind(workspace.stream_id)
        .bind(&upload.path)
        .fetch_one(&mut *tx)
        .await?;

        let head_revision: i32 =
            sqlx::query_scalar("SELECT head_revision FROM files WHERE id = $1 FOR UPDATE")
                .bind(file_id)
                .fetch_one(&mut *tx)
                .await?;

        let existing_action: Option<String> = sqlx::query_scalar(
            "SELECT action FROM changelist_files WHERE changelist_id = $1 AND depot_path = $2",
        )
        .bind(changelist_id)
        .bind(&upload.path)
        .fetch_optional(&mut *tx)
        .await?;
        let action = existing_action.unwrap_or_else(|| {
            if head_revision == 0 {
                "add".to_string()
            } else {
                "edit".to_string()
            }
        });
        if action == "add" && head_revision > 0 {
            return Err(AppError::conflict(format!(
                "{} already exists; use edit instead of add",
                upload.path
            )));
        }

        let revision_number = head_revision + 1;
        let revision_row = sqlx::query(
            r#"
            INSERT INTO file_revisions
                (file_id, stream_id, depot_path, revision_number, blob_hash, size_bytes, action, changelist_id, submitted_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(file_id)
        .bind(workspace.stream_id)
        .bind(&upload.path)
        .bind(revision_number)
        .bind(&upload.manifest.hash)
        .bind(upload.manifest.size_bytes as i64)
        .bind(&action)
        .bind(changelist_id)
        .bind(user.user_id)
        .fetch_one(&mut *tx)
        .await?;
        let revision_id: Uuid = revision_row.get("id");

        sqlx::query("UPDATE files SET head_revision = $1, deleted = FALSE, updated_at = now() WHERE id = $2")
            .bind(revision_number)
            .bind(file_id)
            .execute(&mut *tx)
            .await?;
        record_workspace_revision(&mut tx, workspace.id, &upload.path, revision_number).await?;

        sqlx::query(
            r#"
            INSERT INTO changelist_files (changelist_id, depot_path, action, file_revision_id)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (changelist_id, depot_path)
            DO UPDATE SET file_revision_id = EXCLUDED.file_revision_id, action = EXCLUDED.action
            "#,
        )
        .bind(changelist_id)
        .bind(&upload.path)
        .bind(&action)
        .bind(revision_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE locks
            SET state = 'released', released_at = now()
            WHERE stream_id = $1 AND workspace_id = $2 AND user_id = $3 AND depot_path = $4 AND state = 'active'
            "#,
        )
        .bind(workspace.stream_id)
        .bind(workspace.id)
        .bind(user.user_id)
        .bind(&upload.path)
        .execute(&mut *tx)
        .await?;

        revisions.push(SubmittedRevision {
            path: upload.path,
            revision_number,
            blob_hash: upload.manifest.hash,
            size_bytes: upload.manifest.size_bytes as i64,
        });
    }

    for path in delete_paths {
        let rule = matcher.match_path(&path)?;
        if rule.lock_required
            && !user_holds_lock(
                &mut tx,
                workspace.stream_id,
                workspace.id,
                user.user_id,
                &path,
            )
            .await?
        {
            return Err(AppError::conflict(format!(
                "{path} requires an active exclusive lock held by the submitting user"
            )));
        }

        let current = sqlx::query(
            r#"
            SELECT f.id, f.head_revision, fr.blob_hash, fr.size_bytes
            FROM files f
            JOIN file_revisions fr
              ON fr.file_id = f.id AND fr.revision_number = f.head_revision
            WHERE f.stream_id = $1 AND f.depot_path = $2 AND f.deleted = FALSE
            FOR UPDATE OF f
            "#,
        )
        .bind(workspace.stream_id)
        .bind(&path)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("file not found: {path}")))?;
        let file_id: Uuid = current.get("id");
        let revision_number: i32 = current.get::<i32, _>("head_revision") + 1;
        let blob_hash: String = current.get("blob_hash");
        let size_bytes: i64 = current.get("size_bytes");

        let revision_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO file_revisions
                (file_id, stream_id, depot_path, revision_number, blob_hash, size_bytes, action, changelist_id, submitted_by)
            VALUES ($1, $2, $3, $4, $5, $6, 'delete', $7, $8)
            RETURNING id
            "#,
        )
        .bind(file_id)
        .bind(workspace.stream_id)
        .bind(&path)
        .bind(revision_number)
        .bind(&blob_hash)
        .bind(size_bytes)
        .bind(changelist_id)
        .bind(user.user_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE files SET head_revision = $1, deleted = TRUE, updated_at = now() WHERE id = $2",
        )
        .bind(revision_number)
        .bind(file_id)
        .execute(&mut *tx)
        .await?;
        record_workspace_revision(&mut tx, workspace.id, &path, revision_number).await?;
        sqlx::query(
            "UPDATE changelist_files SET file_revision_id = $1 WHERE changelist_id = $2 AND depot_path = $3",
        )
        .bind(revision_id)
        .bind(changelist_id)
        .bind(&path)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE locks
            SET state = 'released', released_at = now()
            WHERE stream_id = $1 AND workspace_id = $2 AND user_id = $3
              AND depot_path = $4 AND state = 'active'
            "#,
        )
        .bind(workspace.stream_id)
        .bind(workspace.id)
        .bind(user.user_id)
        .bind(&path)
        .execute(&mut *tx)
        .await?;

        revisions.push(SubmittedRevision {
            path,
            revision_number,
            blob_hash,
            size_bytes,
        });
    }

    sqlx::query("UPDATE changelists SET status = 'submitted', submitted_at = now() WHERE id = $1")
        .bind(changelist_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(revisions)
}

async fn submitted_retry_result(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    changelist_id: Uuid,
    uploads: &[SubmittedUpload],
) -> AppResult<Vec<SubmittedRevision>> {
    let rows = sqlx::query(
        r#"
        SELECT cf.depot_path, cf.action, fr.revision_number, fr.blob_hash, fr.size_bytes
        FROM changelist_files cf
        JOIN file_revisions fr ON fr.id = cf.file_revision_id
        WHERE cf.changelist_id = $1
        ORDER BY cf.depot_path
        "#,
    )
    .bind(changelist_id)
    .fetch_all(&mut **tx)
    .await?;
    let content = rows
        .iter()
        .filter(|row| row.get::<String, _>("action") != "delete")
        .map(|row| {
            (
                row.get::<String, _>("depot_path"),
                row.get::<String, _>("blob_hash"),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    if content.len() != uploads.len()
        || uploads.iter().any(|upload| {
            content.get(&upload.path).map(String::as_str) != Some(upload.manifest.hash.as_str())
        })
    {
        return Err(AppError::conflict(
            "submitted changelist retry does not match the original content",
        ));
    }
    Ok(rows
        .into_iter()
        .map(|row| SubmittedRevision {
            path: row.get("depot_path"),
            revision_number: row.get("revision_number"),
            blob_hash: row.get("blob_hash"),
            size_bytes: row.get("size_bytes"),
        })
        .collect())
}

async fn ensure_pending_changelist_for_workspace(
    db: &sqlx::PgPool,
    changelist_id: Uuid,
    user_id: Uuid,
    workspace_id: Uuid,
) -> AppResult<()> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM changelists WHERE id = $1 AND user_id = $2 AND workspace_id = $3",
    )
    .bind(changelist_id)
    .bind(user_id)
    .bind(workspace_id)
    .fetch_optional(db)
    .await?;

    match status.as_deref() {
        Some("pending") => Ok(()),
        Some(_) => Err(AppError::conflict(
            "only pending changelists can be modified",
        )),
        None => Err(AppError::NotFound("changelist not found".to_string())),
    }
}

pub(crate) async fn insert_blob_records(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    manifest: &BlobManifest,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO blobs (hash, size_bytes, chunk_count) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(&manifest.hash)
    .bind(manifest.size_bytes as i64)
    .bind(manifest.chunks.len() as i32)
    .execute(&mut **tx)
    .await?;

    for (index, chunk) in manifest.chunks.iter().enumerate() {
        sqlx::query("INSERT INTO chunks (hash, size_bytes) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(&chunk.hash)
            .bind(chunk.size_bytes as i64)
            .execute(&mut **tx)
            .await?;
        sqlx::query(
            r#"
            INSERT INTO blob_chunks (blob_hash, chunk_hash, chunk_index, size_bytes)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&manifest.hash)
        .bind(&chunk.hash)
        .bind(index as i32)
        .bind(chunk.size_bytes as i64)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn record_workspace_revision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    path: &str,
    revision_number: i32,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO workspace_file_states (workspace_id, depot_path, revision_number, synced_at)
        VALUES ($1, $2, $3, now())
        ON CONFLICT (workspace_id, depot_path)
        DO UPDATE SET revision_number = EXCLUDED.revision_number, synced_at = now()
        "#,
    )
    .bind(workspace_id)
    .bind(path)
    .bind(revision_number)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn changelist_response(row: sqlx::postgres::PgRow) -> ChangelistResponse {
    ChangelistResponse {
        id: row.get("id"),
        user_id: row.get("user_id"),
        workspace_id: row.get("workspace_id"),
        description: row.get("description"),
        status: row.get("status"),
        created_at: row.get("created_at"),
    }
}
