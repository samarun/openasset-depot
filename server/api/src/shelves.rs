//! Shelving: parking pending work on the server without submitting it.
//!
//! A shelf is the set of `shelved_changes` rows belonging to one changelist. It
//! deliberately does not create revisions, move `files.head_revision`, or
//! release locks — the artist is stepping away, not publishing. That is what
//! makes it safe to shelve a scene that does not validate yet.

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Response},
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
    changelists::insert_blob_records,
    error::{AppError, AppResult},
    idempotency,
    paths::normalize_depot_path,
    permissions::{ensure_depot_permission, DepotPermission},
    storage::BlobManifest,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ShelvedFile {
    pub path: String,
    pub blob_hash: String,
    pub size_bytes: i64,
    pub action: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShelfResponse {
    pub changelist_id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub files: Vec<ShelvedFile>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscardShelfResponse {
    pub changelist_id: Uuid,
    pub discarded: i64,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceShelvesQuery {
    pub workspace_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ShelfSummary {
    pub changelist_id: Uuid,
    pub description: String,
    pub changelist_status: String,
    pub user_id: Uuid,
    pub owner: String,
    pub file_count: i64,
    pub total_bytes: i64,
    pub shelved_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnshelveRequest {
    pub changelist_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ShelfContentQuery {
    pub changelist_id: Uuid,
    pub path: String,
}

#[derive(Debug, Deserialize)]
struct StagedShelf {
    path: String,
    blob_hash: String,
}

struct ShelfContext {
    changelist_id: Uuid,
    workspace_id: Uuid,
    stream_id: Uuid,
    owner_user_id: Uuid,
    status: String,
}

/// Uploads pending file content and parks it against a changelist.
///
/// Multipart shape matches submit on purpose, so a host integration can reuse
/// the same upload code: `file` parts named by depot path, plus an optional
/// `staged` array of hashes already in the chunk store from a resumable upload.
pub async fn shelve_changelist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(changelist_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<ShelfResponse>> {
    let user = state.require_user(&headers)?;
    let _storage_lease = state.storage_maintenance.read().await;
    let context = shelf_context(&state.db, changelist_id, &user, DepotPermission::Write).await?;
    if context.owner_user_id != user.user_id {
        return Err(AppError::Forbidden(
            "only the changelist owner can shelve its files".to_string(),
        ));
    }
    if context.status != "pending" {
        return Err(AppError::conflict(
            "only pending changelists can be shelved",
        ));
    }

    let mut uploads: Vec<(String, BlobManifest)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while let Some(mut field) = multipart.next_field().await? {
        let name = field.name().unwrap_or_default().to_string();

        if name == "staged" {
            let value = field.text().await?;
            let staged: Vec<StagedShelf> = serde_json::from_str(&value)
                .map_err(|error| AppError::bad_request(format!("invalid staged field: {error}")))?;
            for entry in staged {
                let depot_path = normalize_depot_path(&entry.path)?;
                if !seen.insert(depot_path.to_lowercase()) {
                    return Err(AppError::bad_request(format!(
                        "duplicate file part for depot path {depot_path}"
                    )));
                }
                let manifest = state.storage.read_manifest(&entry.blob_hash).await?;
                uploads.push((depot_path, manifest));
            }
            continue;
        }

        if name == "file" {
            let file_name = field
                .file_name()
                .ok_or_else(|| AppError::bad_request("file part is missing a file name"))?
                .to_string();
            let depot_path = normalize_depot_path(&file_name)?;
            if !seen.insert(depot_path.to_lowercase()) {
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
            uploads.push((depot_path, result?));
        }
    }

    if uploads.is_empty() {
        return Err(AppError::bad_request(
            "a shelf needs at least one file part",
        ));
    }

    let mut tx = state.db.begin().await?;
    for (path, manifest) in &uploads {
        insert_blob_records(&mut tx, manifest).await?;

        // Whether this is an add or an edit depends on the depot as it stands
        // now, which is the honest answer: if the path already exists, the
        // shelved bytes are a change to it.
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM files WHERE stream_id = $1 AND depot_path = $2 AND deleted = FALSE)",
        )
        .bind(context.stream_id)
        .bind(path)
        .fetch_one(&mut *tx)
        .await?;
        let action = if exists { "edit" } else { "add" };

        sqlx::query(
            r#"
            INSERT INTO shelved_changes
                (changelist_id, workspace_id, stream_id, user_id, depot_path, blob_hash, size_bytes, action, state)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active')
            ON CONFLICT (changelist_id, depot_path) DO UPDATE
            SET blob_hash = EXCLUDED.blob_hash,
                size_bytes = EXCLUDED.size_bytes,
                action = EXCLUDED.action,
                state = 'active',
                updated_at = now()
            "#,
        )
        .bind(changelist_id)
        .bind(context.workspace_id)
        .bind(context.stream_id)
        .bind(user.user_id)
        .bind(path)
        .bind(&manifest.hash)
        .bind(manifest.size_bytes as i64)
        .bind(action)
        .execute(&mut *tx)
        .await?;
    }

    audit::record_tx(
        &mut tx,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(context.stream_id),
            workspace_id: Some(context.workspace_id),
            changelist_id: Some(changelist_id),
            ..audit::AuditEvent::new(
                "changelist_shelve",
                serde_json::json!({ "file_count": uploads.len() }),
            )
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(read_shelf(&state.db, &context).await?))
}

/// Lists the files currently parked against a changelist.
pub async fn get_shelf(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(changelist_id): Path<Uuid>,
) -> AppResult<Json<ShelfResponse>> {
    let user = state.require_user(&headers)?;
    let context = shelf_context(&state.db, changelist_id, &user, DepotPermission::Read).await?;
    Ok(Json(read_shelf(&state.db, &context).await?))
}

/// Lists every shelf in a workspace's stream, newest first.
///
/// Scoped to the stream rather than the caller's own workspaces so a lead can
/// see what the team has parked.
pub async fn list_shelves(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WorkspaceShelvesQuery>,
) -> AppResult<Json<Vec<ShelfSummary>>> {
    let user = state.require_user(&headers)?;
    let workspace =
        crate::workspaces::workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let rows = sqlx::query(
        r#"
        SELECT s.changelist_id,
               c.description,
               c.status AS changelist_status,
               s.user_id,
               u.username AS owner,
               count(*) AS file_count,
               -- sum() over bigint yields numeric, which will not decode as i64.
               coalesce(sum(s.size_bytes), 0)::bigint AS total_bytes,
               max(s.updated_at) AS shelved_at
        FROM shelved_changes s
        JOIN changelists c ON c.id = s.changelist_id
        JOIN users u ON u.id = s.user_id
        WHERE s.stream_id = $1 AND s.state = 'active'
        GROUP BY s.changelist_id, c.description, c.status, s.user_id, u.username
        ORDER BY max(s.updated_at) DESC
        LIMIT 500
        "#,
    )
    .bind(workspace.stream_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|row| ShelfSummary {
                changelist_id: row.get("changelist_id"),
                description: row.get("description"),
                changelist_status: row.get("changelist_status"),
                user_id: row.get("user_id"),
                owner: row.get("owner"),
                file_count: row.get("file_count"),
                total_bytes: row.get("total_bytes"),
                shelved_at: row.get("shelved_at"),
            })
            .collect(),
    ))
}

/// Restores a shelf's paths as pending file operations on its changelist.
///
/// This returns the file list rather than the bytes; the client downloads each
/// one from `/api/shelves/content`, which supports ranges so a large scene
/// resumes like any other transfer.
pub async fn unshelve_changelist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(changelist_id): Path<Uuid>,
) -> AppResult<Json<ShelfResponse>> {
    let user = state.require_user(&headers)?;
    let context = shelf_context(&state.db, changelist_id, &user, DepotPermission::Write).await?;
    if context.owner_user_id != user.user_id {
        return Err(AppError::Forbidden(
            "only the changelist owner can unshelve its files".to_string(),
        ));
    }
    if context.status != "pending" {
        return Err(AppError::conflict(
            "only pending changelists can be unshelved",
        ));
    }

    let request = UnshelveRequest { changelist_id };
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "changelist_unshelve",
        &request,
        || async {
            let mut tx = state.db.begin().await?;
            let rows = sqlx::query(
                r#"
                UPDATE shelved_changes
                SET state = 'unshelved', updated_at = now()
                WHERE changelist_id = $1 AND state = 'active'
                RETURNING depot_path, action
                "#,
            )
            .bind(changelist_id)
            .fetch_all(&mut *tx)
            .await?;
            if rows.is_empty() {
                return Err(AppError::NotFound(
                    "no active shelved files for this changelist".to_string(),
                ));
            }

            for row in &rows {
                let depot_path: String = row.get("depot_path");
                let action: String = row.get("action");
                sqlx::query(
                    r#"
                    INSERT INTO changelist_files (changelist_id, depot_path, action)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (changelist_id, depot_path) DO UPDATE SET action = EXCLUDED.action
                    "#,
                )
                .bind(changelist_id)
                .bind(&depot_path)
                .bind(&action)
                .execute(&mut *tx)
                .await?;
            }

            audit::record_tx(
                &mut tx,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(context.stream_id),
                    workspace_id: Some(context.workspace_id),
                    changelist_id: Some(changelist_id),
                    ..audit::AuditEvent::new(
                        "changelist_unshelve",
                        serde_json::json!({ "file_count": rows.len() }),
                    )
                },
            )
            .await?;
            tx.commit().await?;

            // Reports the rows just restored, which `read_shelf` would no longer
            // return now that they have left the active state.
            let files = sqlx::query(
                r#"
                SELECT depot_path, blob_hash, size_bytes, action, created_at
                FROM shelved_changes
                WHERE changelist_id = $1 AND state = 'unshelved'
                ORDER BY depot_path
                "#,
            )
            .bind(changelist_id)
            .fetch_all(&state.db)
            .await?;
            Ok(ShelfResponse {
                changelist_id,
                workspace_id: context.workspace_id,
                user_id: context.owner_user_id,
                files: files.into_iter().map(shelved_file).collect(),
            })
        },
    )
    .await?;

    Ok(Json(response))
}

/// Discards a shelf without restoring it.
///
/// The rows are marked `deleted` rather than removed so the audit trail still
/// explains where an artist's parked work went. Blobs stay in the store for
/// orphan cleanup to reclaim once nothing references them.
pub async fn discard_shelf(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(changelist_id): Path<Uuid>,
) -> AppResult<Json<DiscardShelfResponse>> {
    let user = state.require_user(&headers)?;
    let context = shelf_context(&state.db, changelist_id, &user, DepotPermission::Write).await?;
    if context.owner_user_id != user.user_id && !user.is_admin {
        return Err(AppError::Forbidden(
            "only the changelist owner or an admin can discard a shelf".to_string(),
        ));
    }

    let request = UnshelveRequest { changelist_id };
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "changelist_shelf_discard",
        &request,
        || async {
            let mut tx = state.db.begin().await?;
            let discarded = sqlx::query(
                "UPDATE shelved_changes SET state = 'deleted', updated_at = now() WHERE changelist_id = $1 AND state = 'active'",
            )
            .bind(changelist_id)
            .execute(&mut *tx)
            .await?
            .rows_affected() as i64;
            if discarded == 0 {
                return Err(AppError::NotFound(
                    "no active shelved files for this changelist".to_string(),
                ));
            }
            audit::record_tx(
                &mut tx,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(context.stream_id),
                    workspace_id: Some(context.workspace_id),
                    changelist_id: Some(changelist_id),
                    ..audit::AuditEvent::new(
                        "changelist_shelf_discard",
                        serde_json::json!({ "file_count": discarded }),
                    )
                },
            )
            .await?;
            tx.commit().await?;
            Ok(DiscardShelfResponse {
                changelist_id,
                discarded,
            })
        },
    )
    .await?;

    Ok(Json(response))
}

/// Streams one shelved file's bytes, honouring `Range` for resumed transfers.
pub async fn download_shelf_content(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ShelfContentQuery>,
) -> AppResult<Response<Body>> {
    let user = state.require_user(&headers)?;
    let context =
        shelf_context(&state.db, query.changelist_id, &user, DepotPermission::Read).await?;
    let path = normalize_depot_path(&query.path)?;

    // Content stays readable after unshelving so a client that restored the
    // list but lost the download can still finish pulling the bytes.
    let row = sqlx::query(
        r#"
        SELECT blob_hash, size_bytes
        FROM shelved_changes
        WHERE changelist_id = $1 AND depot_path = $2 AND state <> 'deleted'
        "#,
    )
    .bind(context.changelist_id)
    .bind(&path)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("shelved file not found".to_string()))?;
    let blob_hash: String = row.get("blob_hash");
    let total_size = row.get::<i64, _>("size_bytes").max(0) as u64;

    let mut response = match crate::sync::parse_byte_range(&headers, total_size)? {
        Some((start, end_inclusive)) => {
            let body = Body::from_stream(state.storage.stream_blob_range(
                blob_hash,
                start,
                end_inclusive,
            ));
            let mut response = Response::new(body);
            *response.status_mut() = axum::http::StatusCode::PARTIAL_CONTENT;
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end_inclusive}/{total_size}"))
                    .map_err(|_| AppError::internal("invalid response header"))?,
            );
            response.headers_mut().insert(
                header::CONTENT_LENGTH,
                HeaderValue::from(end_inclusive - start + 1),
            );
            response
        }
        None => {
            let mut response =
                Response::new(Body::from_stream(state.storage.stream_blob(blob_hash)));
            response
                .headers_mut()
                .insert(header::CONTENT_LENGTH, HeaderValue::from(total_size));
            response
        }
    };
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response.headers_mut().insert(
        "x-oad-depot-path",
        HeaderValue::from_str(&path).map_err(|_| AppError::internal("invalid response header"))?,
    );
    Ok(response)
}

async fn read_shelf(db: &sqlx::PgPool, context: &ShelfContext) -> AppResult<ShelfResponse> {
    let rows = sqlx::query(
        r#"
        SELECT depot_path, blob_hash, size_bytes, action, created_at
        FROM shelved_changes
        WHERE changelist_id = $1 AND state = 'active'
        ORDER BY depot_path
        "#,
    )
    .bind(context.changelist_id)
    .fetch_all(db)
    .await?;
    Ok(ShelfResponse {
        changelist_id: context.changelist_id,
        workspace_id: context.workspace_id,
        user_id: context.owner_user_id,
        files: rows.into_iter().map(shelved_file).collect(),
    })
}

fn shelved_file(row: sqlx::postgres::PgRow) -> ShelvedFile {
    ShelvedFile {
        path: row.get("depot_path"),
        blob_hash: row.get("blob_hash"),
        size_bytes: row.get("size_bytes"),
        action: row.get("action"),
        created_at: row.get("created_at"),
    }
}

/// Resolves a changelist to its stream and checks depot access.
///
/// Deliberately does not go through `workspace_for_user`: a shelf is readable by
/// anyone with depot access, not just the workspace owner, so a teammate can see
/// what has been parked. Write operations layer an ownership check on top.
async fn shelf_context(
    db: &sqlx::PgPool,
    changelist_id: Uuid,
    user: &AuthUser,
    required: DepotPermission,
) -> AppResult<ShelfContext> {
    let row = sqlx::query(
        r#"
        SELECT c.id, c.user_id, c.status, w.id AS workspace_id, w.stream_id, w.depot_id
        FROM changelists c
        JOIN workspaces w ON w.id = c.workspace_id
        WHERE c.id = $1
        "#,
    )
    .bind(changelist_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::NotFound("changelist not found".to_string()))?;

    let depot_id: Uuid = row.get("depot_id");
    ensure_depot_permission(db, user, depot_id, required).await?;
    Ok(ShelfContext {
        changelist_id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        stream_id: row.get("stream_id"),
        owner_user_id: row.get("user_id"),
        status: row.get("status"),
    })
}
