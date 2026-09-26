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
    idempotency,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct SyncAckRequest {
    pub workspace_id: Uuid,
    pub entries: Vec<SyncAckEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SyncAckEntry {
    pub path: String,
    pub revision_number: i32,
}

#[derive(Debug, Deserialize, Serialize)]
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
    for entry in &req.entries {
        if entry.revision_number <= 0 {
            return Err(AppError::bad_request("revision_number must be positive"));
        }
        paths.push(normalize_depot_path(&entry.path)?);
        revisions.push(entry.revision_number);
    }

    // A retried acknowledgement is harmless on its own, but a client that lost
    // the response cannot tell an accepted batch from one the stream moved past.
    // Replaying the original answer keeps that distinction meaningful.
    let response = idempotency::run(&state.db, &headers, &user, "sync_ack", &req, || async {
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
        Ok(SyncAckResponse {
            accepted: paths.len() as u64,
        })
    })
    .await?;

    Ok(Json(response))
}

pub async fn download_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DownloadRequest>,
) -> AppResult<Response<Body>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let path = normalize_depot_path(&req.path)?;
    let row = sqlx::query(
        r#"
        SELECT fr.blob_hash, fr.size_bytes
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
    let blob_hash: String = row.get("blob_hash");
    let total_size = row.get::<i64, _>("size_bytes").max(0) as u64;

    // A client resuming an interrupted download asks for the bytes it is still
    // missing. Content is immutable and content-addressed, so a range served
    // now is guaranteed to belong to the same revision the client started.
    let range = parse_byte_range(&headers, total_size)?;

    let mut response = match range {
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

/// Parses a single-range `Range: bytes=…` header into inclusive bounds.
///
/// Returns `Ok(None)` when no range was requested. Multi-range requests are
/// deliberately unsupported — resumable sync only ever needs a single open-ended
/// suffix — and anything unsatisfiable is rejected rather than silently served
/// in full, so a resuming client cannot corrupt its local file.
pub(crate) fn parse_byte_range(
    headers: &HeaderMap,
    total_size: u64,
) -> AppResult<Option<(u64, u64)>> {
    let Some(value) = headers.get(header::RANGE) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| AppError::bad_request("Range header must be valid ASCII"))?
        .trim();
    let Some(spec) = value.strip_prefix("bytes=") else {
        return Err(AppError::bad_request("only byte ranges are supported"));
    };
    if spec.contains(',') {
        return Err(AppError::bad_request(
            "only a single byte range is supported",
        ));
    }
    let (start_text, end_text) = spec
        .split_once('-')
        .ok_or_else(|| AppError::bad_request("malformed byte range"))?;

    let (start, end_inclusive) = match (start_text.trim(), end_text.trim()) {
        // "bytes=-N": the final N bytes.
        ("", suffix) => {
            let length: u64 = suffix
                .parse()
                .map_err(|_| AppError::bad_request("malformed byte range"))?;
            if length == 0 {
                return Err(unsatisfiable(total_size));
            }
            (
                total_size.saturating_sub(length),
                total_size.saturating_sub(1),
            )
        }
        (start, "") => {
            let start: u64 = start
                .parse()
                .map_err(|_| AppError::bad_request("malformed byte range"))?;
            (start, total_size.saturating_sub(1))
        }
        (start, end) => {
            let start: u64 = start
                .parse()
                .map_err(|_| AppError::bad_request("malformed byte range"))?;
            let end: u64 = end
                .parse()
                .map_err(|_| AppError::bad_request("malformed byte range"))?;
            (start, end.min(total_size.saturating_sub(1)))
        }
    };

    if total_size == 0 || start >= total_size || start > end_inclusive {
        return Err(unsatisfiable(total_size));
    }
    Ok(Some((start, end_inclusive)))
}

fn unsatisfiable(total_size: u64) -> AppError {
    AppError::bad_request(format!(
        "requested range is not satisfiable for a {total_size} byte object"
    ))
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

#[cfg(test)]
mod tests {
    use axum::http::{header, HeaderMap, HeaderValue};

    use super::parse_byte_range;

    fn range_headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn absent_range_streams_the_whole_object() {
        assert_eq!(parse_byte_range(&HeaderMap::new(), 100).unwrap(), None);
    }

    #[test]
    fn open_ended_range_resumes_to_the_end() {
        assert_eq!(
            parse_byte_range(&range_headers("bytes=40-"), 100).unwrap(),
            Some((40, 99))
        );
    }

    #[test]
    fn closed_range_is_clamped_to_the_object_size() {
        assert_eq!(
            parse_byte_range(&range_headers("bytes=10-20"), 100).unwrap(),
            Some((10, 20))
        );
        assert_eq!(
            parse_byte_range(&range_headers("bytes=90-4000"), 100).unwrap(),
            Some((90, 99))
        );
    }

    #[test]
    fn suffix_range_returns_the_final_bytes() {
        assert_eq!(
            parse_byte_range(&range_headers("bytes=-10"), 100).unwrap(),
            Some((90, 99))
        );
    }

    #[test]
    fn a_client_that_already_has_every_byte_is_rejected_rather_than_resent() {
        assert!(parse_byte_range(&range_headers("bytes=100-"), 100).is_err());
        assert!(parse_byte_range(&range_headers("bytes=0-"), 0).is_err());
    }

    #[test]
    fn unsupported_and_malformed_ranges_are_rejected() {
        assert!(parse_byte_range(&range_headers("items=0-10"), 100).is_err());
        assert!(parse_byte_range(&range_headers("bytes=0-10,20-30"), 100).is_err());
        assert!(parse_byte_range(&range_headers("bytes=abc-"), 100).is_err());
        assert!(parse_byte_range(&range_headers("bytes=5"), 100).is_err());
    }
}
