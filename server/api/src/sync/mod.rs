use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    error::{AppError, AppResult},
    paths::normalize_depot_path,
    workspaces::workspace_for_user,
};

#[derive(Debug, Deserialize)]
pub struct SyncPlanRequest {
    pub workspace_id: Uuid,
    pub path_prefix: Option<String>,
    pub after_path: Option<String>,
    pub limit: Option<i64>,
    pub paths: Option<Vec<String>>,
    #[serde(default)]
    pub include_current: bool,
    #[serde(default)]
    pub force_full: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncPlanEntry {
    pub path: String,
    pub revision_number: i32,
    pub blob_hash: String,
    pub size_bytes: i64,
    pub deleted: bool,
    pub preview_available: bool,
    pub review_proxy_available: bool,
}

#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub workspace_id: Uuid,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct SyncAckRequest {
    pub workspace_id: Uuid,
    pub entries: Vec<SyncAckEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SyncAckEntry {
    pub path: String,
    pub revision_number: i32,
}

#[derive(Debug, Serialize)]
pub struct SyncAckResponse {
    pub accepted: u64,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub workspace_id: Uuid,
    pub path: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct HistoryEntry {
    pub revision_number: i32,
    pub blob_hash: String,
    pub size_bytes: i64,
    pub action: String,
    pub submitted_by: Uuid,
    pub submitted_at: DateTime<Utc>,
}

pub async fn plan_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SyncPlanRequest>,
) -> AppResult<Json<Vec<SyncPlanEntry>>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let prefix = match req.path_prefix {
        Some(value) => Some(normalize_depot_path(&value)?),
        None => None,
    };
    let after_path = match req.after_path {
        Some(value) => Some(normalize_depot_path(&value)?),
        None => None,
    };
    let paths = match req.paths {
        Some(paths) if paths.len() > 5_000 => {
            return Err(AppError::bad_request(
                "sync plan accepts at most 5000 paths",
            ));
        }
        Some(paths) => Some(
            paths
                .into_iter()
                .map(|path| normalize_depot_path(&path))
                .collect::<AppResult<Vec<_>>>()?,
        ),
        None => None,
    };
    let limit = req.limit.unwrap_or(1_000).clamp(1, 5_000);
    let rows = sqlx::query(
        r#"
        SELECT f.depot_path, f.head_revision, fr.blob_hash, fr.size_bytes, f.deleted,
               (rp.revision_id IS NOT NULL) AS preview_available,
               (rrp.revision_id IS NOT NULL) AS review_proxy_available
        FROM files f
        JOIN file_revisions fr ON fr.file_id = f.id AND fr.revision_number = f.head_revision
        LEFT JOIN revision_previews rp ON rp.revision_id = fr.id
        LEFT JOIN revision_review_proxies rrp ON rrp.revision_id = fr.id
        LEFT JOIN workspace_file_states wfs
          ON wfs.workspace_id = $2 AND wfs.depot_path = f.depot_path
        WHERE f.stream_id = $1
          AND ($3::text IS NULL OR f.depot_path LIKE ($3 || '%'))
          AND ($4::text IS NULL OR f.depot_path > $4)
          AND ($5::text[] IS NULL OR f.depot_path = ANY($5))
          AND ($6::boolean OR $7::boolean OR wfs.revision_number IS DISTINCT FROM f.head_revision)
        ORDER BY f.depot_path ASC
        LIMIT $8
        "#,
    )
    .bind(workspace.stream_id)
    .bind(workspace.id)
    .bind(prefix)
    .bind(after_path)
    .bind(paths)
    .bind(req.include_current)
    .bind(req.force_full)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|row| SyncPlanEntry {
                path: row.get("depot_path"),
                revision_number: row.get("head_revision"),
                blob_hash: row.get("blob_hash"),
                size_bytes: row.get("size_bytes"),
                deleted: row.get("deleted"),
                preview_available: row.get("preview_available"),
                review_proxy_available: row.get("review_proxy_available"),
            })
            .collect(),
    ))
}

pub async fn ack_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SyncAckRequest>,
) -> AppResult<Json<SyncAckResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    if req.entries.is_empty() || req.entries.len() > 5_000 {
        return Err(AppError::bad_request(
            "sync acknowledgement requires 1 to 5000 entries",
        ));
    }
    let mut paths = Vec::with_capacity(req.entries.len());
    let mut revisions = Vec::with_capacity(req.entries.len());
    for entry in req.entries {
        if entry.revision_number <= 0 {
            return Err(AppError::bad_request("revision_number must be positive"));
        }
        paths.push(normalize_depot_path(&entry.path)?);
        revisions.push(entry.revision_number);
    }

    let result = sqlx::query(
        r#"
        INSERT INTO workspace_file_states (workspace_id, depot_path, revision_number, synced_at)
        SELECT $1, requested.depot_path, requested.revision_number, now()
        FROM UNNEST($2::text[], $3::int[]) AS requested(depot_path, revision_number)
        JOIN files f
          ON f.stream_id = $4
         AND f.depot_path = requested.depot_path
         AND f.head_revision >= requested.revision_number
        ON CONFLICT (workspace_id, depot_path)
        DO UPDATE SET revision_number = EXCLUDED.revision_number, synced_at = now()
        "#,
    )
    .bind(workspace.id)
    .bind(&paths)
    .bind(&revisions)
    .bind(workspace.stream_id)
    .execute(&state.db)
    .await?;
    if result.rows_affected() != paths.len() as u64 {
        return Err(AppError::conflict(
            "one or more acknowledged files no longer match the stream",
        ));
    }
    crate::audit::record(
        &state.db,
        crate::audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(workspace.stream_id),
            workspace_id: Some(workspace.id),
            ..crate::audit::AuditEvent::new(
                "sync_ack",
                serde_json::json!({ "file_count": paths.len() }),
            )
        },
    )
    .await?;
    Ok(Json(SyncAckResponse {
        accepted: paths.len() as u64,
    }))
}

pub async fn download_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DownloadRequest>,
) -> AppResult<Response<Body>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let path = normalize_depot_path(&req.path)?;
    let blob_hash: String = sqlx::query_scalar(
        r#"
        SELECT fr.blob_hash
        FROM files f
        JOIN file_revisions fr ON fr.file_id = f.id AND fr.revision_number = f.head_revision
        WHERE f.stream_id = $1 AND f.depot_path = $2 AND f.deleted = FALSE
        "#,
    )
    .bind(workspace.stream_id)
    .bind(&path)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("file not found".to_string()))?;

    let mut response = Response::new(Body::from_stream(state.storage.stream_blob(blob_hash)));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        "x-oad-depot-path",
        HeaderValue::from_str(&path).map_err(|_| AppError::internal("invalid response header"))?,
    );
    Ok(response)
}

pub async fn file_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> AppResult<Json<Vec<HistoryEntry>>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let path = normalize_depot_path(&query.path)?;
    let limit = query.limit.unwrap_or(100).clamp(1, 1_000);
    let offset = query.offset.unwrap_or(0).max(0);
    let rows = sqlx::query(
        r#"
        SELECT revision_number, blob_hash, size_bytes, action, submitted_by, submitted_at
        FROM file_revisions
        WHERE stream_id = $1 AND depot_path = $2
        ORDER BY revision_number ASC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(workspace.stream_id)
    .bind(path)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|row| HistoryEntry {
                revision_number: row.get("revision_number"),
                blob_hash: row.get("blob_hash"),
                size_bytes: row.get("size_bytes"),
                action: row.get("action"),
                submitted_by: row.get("submitted_by"),
                submitted_at: row.get("submitted_at"),
            })
            .collect(),
    ))
}
