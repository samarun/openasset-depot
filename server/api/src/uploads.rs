//! Resumable staging for large file uploads.
//!
//! Submit is atomic and multipart, so a dropped connection part-way through a
//! large plate or `.umap` previously meant re-sending every byte. An upload
//! session stages one file's bytes across as many requests as it takes, lets a
//! client ask how much the server already holds, and continues from that offset.
//!
//! Staged bytes are ingested into the normal content-addressed chunk store, so
//! deduplication, manifests, and revision commit are unchanged: submit simply
//! references the resulting blob hash instead of carrying the bytes again.

use axum::{
    body::Body,
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    auth::AuthUser,
    error::{AppError, AppResult},
    paths::normalize_depot_path,
    workspaces::workspace_for_user,
};

/// Header carrying the byte offset a chunk request continues from.
const UPLOAD_OFFSET: &str = "x-upload-offset";

#[derive(Debug, Deserialize)]
pub struct BeginUploadRequest {
    pub workspace_id: Uuid,
    pub path: String,
    /// Total size when the client knows it, so overruns are rejected early.
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadSessionResponse {
    pub upload_id: Uuid,
    pub path: String,
    pub received_bytes: i64,
    pub declared_size_bytes: Option<i64>,
    pub blob_hash: Option<String>,
    pub finalized: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinalizeUploadResponse {
    pub upload_id: Uuid,
    pub path: String,
    pub blob_hash: String,
    pub size_bytes: i64,
}

/// Opens a session, or returns the existing open one for the same target.
///
/// Returning the existing session is what makes a retry cheap: a client that
/// lost its upload id after a crash re-begins with the same workspace and path
/// and learns the offset to resume from.
pub async fn begin_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BeginUploadRequest>,
) -> AppResult<Json<UploadSessionResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let depot_path = normalize_depot_path(&req.path)?;
    if let Some(size) = req.size_bytes {
        if size < 0 {
            return Err(AppError::bad_request("size_bytes cannot be negative"));
        }
        let max = state.config.max_upload_bytes as i64;
        if size > max {
            return Err(AppError::PayloadTooLarge(format!(
                "{depot_path} is larger than the {max} byte upload limit"
            )));
        }
    }

    let row = sqlx::query(
        r#"
        INSERT INTO upload_sessions (user_id, workspace_id, depot_path, declared_size_bytes)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (workspace_id, depot_path) WHERE finalized_at IS NULL
        DO UPDATE SET updated_at = now()
        RETURNING id, depot_path, received_bytes, declared_size_bytes, blob_hash,
                  finalized_at, created_at
        "#,
    )
    .bind(user.user_id)
    .bind(workspace.id)
    .bind(&depot_path)
    .bind(req.size_bytes)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(session_response(row)))
}

/// Reports how many bytes the server holds, so a client can resume.
pub async fn get_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(upload_id): Path<Uuid>,
) -> AppResult<Json<UploadSessionResponse>> {
    let user = state.require_user(&headers)?;
    let row = load_session(&state, &user, upload_id).await?;
    Ok(Json(session_response(row)))
}

/// Appends a slice of the file at the offset the client claims to be at.
///
/// The offset is verified against the staged length rather than trusted: a
/// mismatch means the client and server disagree about progress, and silently
/// writing at the wrong place would corrupt the file while still producing a
/// valid-looking blob.
pub async fn upload_chunk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(upload_id): Path<Uuid>,
    body: Body,
) -> AppResult<Json<UploadSessionResponse>> {
    let user = state.require_user(&headers)?;
    let row = load_session(&state, &user, upload_id).await?;
    if row
        .get::<Option<DateTime<Utc>>, _>("finalized_at")
        .is_some()
    {
        return Err(AppError::conflict("upload session is already finalized"));
    }
    let received: i64 = row.get("received_bytes");
    let declared: Option<i64> = row.get("declared_size_bytes");
    let offset = requested_offset(&headers)?.unwrap_or(received);
    if offset != received {
        return Err(AppError::conflict(format!(
            "upload is at byte {received}, not {offset}; resume from {received}"
        )));
    }

    let staging = state.storage.temp_upload_path(upload_id);
    if let Some(parent) = staging.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        // Never truncate on open: the bytes already staged are what makes the
        // upload resumable. The explicit set_len below trims only an unrecorded tail.
        .truncate(false)
        .open(&staging)
        .await?;
    // Truncating to the recorded length discards any tail written by a request
    // that failed after its bytes hit the disk but before the row was updated.
    file.set_len(received as u64).await?;
    file.seek(std::io::SeekFrom::Start(received as u64)).await?;

    let limit = state.config.max_upload_bytes as i64;
    let mut written = received;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|error| AppError::bad_request(format!("upload failed: {error}")))?;
        written += chunk.len() as i64;
        if written > limit {
            drop(file);
            return Err(AppError::PayloadTooLarge(format!(
                "upload exceeds the {limit} byte limit"
            )));
        }
        if let Some(declared) = declared {
            if written > declared {
                drop(file);
                return Err(AppError::bad_request(format!(
                    "upload exceeds the declared size of {declared} bytes"
                )));
            }
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    let row = sqlx::query(
        r#"
        UPDATE upload_sessions
        SET received_bytes = $2, updated_at = now()
        WHERE id = $1 AND finalized_at IS NULL
        RETURNING id, depot_path, received_bytes, declared_size_bytes, blob_hash,
                  finalized_at, created_at
        "#,
    )
    .bind(upload_id)
    .bind(written)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::conflict("upload session is already finalized"))?;

    Ok(Json(session_response(row)))
}

/// Ingests the staged file into content-addressed storage.
///
/// After this the bytes live in the normal chunk store, so submit can reference
/// the blob hash and the staging file is no longer needed.
pub async fn finalize_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(upload_id): Path<Uuid>,
) -> AppResult<Json<FinalizeUploadResponse>> {
    let user = state.require_user(&headers)?;
    // Held so orphan cleanup cannot remove a chunk between ingest and commit.
    let _storage_lease = state.storage_maintenance.read().await;
    let row = load_session(&state, &user, upload_id).await?;
    let path: String = row.get("depot_path");
    let workspace_id: Uuid = row.get("workspace_id");
    let declared: Option<i64> = row.get("declared_size_bytes");
    let received: i64 = row.get("received_bytes");

    // Replaying finalize returns the original hash instead of failing, so a
    // client that lost the response can safely ask again.
    if let Some(blob_hash) = row.get::<Option<String>, _>("blob_hash") {
        return Ok(Json(FinalizeUploadResponse {
            upload_id,
            path,
            blob_hash,
            size_bytes: received,
        }));
    }

    if let Some(declared) = declared {
        if received != declared {
            return Err(AppError::bad_request(format!(
                "upload holds {received} of {declared} bytes; resume before finalizing"
            )));
        }
    }

    let staging = state.storage.temp_upload_path(upload_id);
    if !tokio::fs::try_exists(&staging).await.unwrap_or(false) {
        return Err(AppError::conflict(
            "staged upload is no longer available; begin the upload again",
        ));
    }
    let manifest = state.storage.put_file(&staging).await?;
    let _ = tokio::fs::remove_file(&staging).await;

    sqlx::query(
        r#"
        UPDATE upload_sessions
        SET blob_hash = $2, finalized_at = now(), updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(upload_id)
    .bind(&manifest.hash)
    .execute(&state.db)
    .await?;

    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            workspace_id: Some(workspace_id),
            depot_path: Some(&path),
            ..audit::AuditEvent::new(
                "upload_finalize",
                serde_json::json!({
                    "upload_id": upload_id,
                    "blob_hash": manifest.hash,
                    "size_bytes": manifest.size_bytes,
                }),
            )
        },
    )
    .await?;

    Ok(Json(FinalizeUploadResponse {
        upload_id,
        path,
        blob_hash: manifest.hash,
        size_bytes: manifest.size_bytes as i64,
    }))
}

/// Loads a session owned by `user`.
///
/// Sessions are private to their creator: staged bytes are unverified content
/// and must not become readable or writable by another artist.
async fn load_session(
    state: &AppState,
    user: &AuthUser,
    upload_id: Uuid,
) -> AppResult<sqlx::postgres::PgRow> {
    sqlx::query(
        r#"
        SELECT id, user_id, workspace_id, depot_path, received_bytes,
               declared_size_bytes, blob_hash, finalized_at, created_at
        FROM upload_sessions
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(upload_id)
    .bind(user.user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("upload session not found".to_string()))
}

fn requested_offset(headers: &HeaderMap) -> AppResult<Option<i64>> {
    let Some(value) = headers.get(UPLOAD_OFFSET) else {
        return Ok(None);
    };
    let offset = value
        .to_str()
        .ok()
        .and_then(|text| text.trim().parse::<i64>().ok())
        .ok_or_else(|| AppError::bad_request("X-Upload-Offset must be a byte count"))?;
    if offset < 0 {
        return Err(AppError::bad_request("X-Upload-Offset cannot be negative"));
    }
    Ok(Some(offset))
}

fn session_response(row: sqlx::postgres::PgRow) -> UploadSessionResponse {
    let finalized_at: Option<DateTime<Utc>> = row.get("finalized_at");
    UploadSessionResponse {
        upload_id: row.get("id"),
        path: row.get("depot_path"),
        received_bytes: row.get("received_bytes"),
        declared_size_bytes: row.get("declared_size_bytes"),
        blob_hash: row.get("blob_hash"),
        finalized: finalized_at.is_some(),
        created_at: row.get("created_at"),
    }
}
