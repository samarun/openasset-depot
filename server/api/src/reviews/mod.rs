use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use tokio_util::io::StreamReader;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    changelists::insert_blob_records,
    error::{AppError, AppResult},
    idempotency,
    paths::normalize_depot_path,
    uploads::finalized_upload_for_target,
    workspaces::workspace_for_user,
};

pub mod requests;

const MAX_ANNOTATION_BYTES: usize = 128 * 1024;
const REVIEW_PROXY_PURPOSE: &str = "review_proxy";
const MAX_FRAME_RATE_NUMERATOR: i32 = 120_000;
const MAX_FRAME_RATE_DENOMINATOR: i32 = 10_000;

#[derive(Debug, Deserialize)]
pub struct ReviewTargetQuery {
    pub workspace_id: Uuid,
    pub path: String,
    pub revision_number: i32,
    pub frame_rate_numerator: Option<i32>,
    pub frame_rate_denominator: Option<i32>,
    pub start_frame: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct AttachReviewProxyRequest {
    pub workspace_id: Uuid,
    pub path: String,
    pub revision_number: i32,
    pub upload_id: Uuid,
    pub content_type: String,
    pub frame_rate_numerator: Option<i32>,
    pub frame_rate_denominator: Option<i32>,
    pub start_frame: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReviewCommentRequest {
    pub workspace_id: Uuid,
    pub path: String,
    pub revision_number: i32,
    pub body: String,
    pub timecode_ms: Option<i64>,
    pub frame_number: Option<i32>,
    pub parent_comment_id: Option<Uuid>,
    pub annotation: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveReviewCommentRequest {
    pub workspace_id: Uuid,
    #[serde(default = "default_true")]
    pub resolved: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewCommentResponse {
    pub id: Uuid,
    pub path: String,
    pub revision_number: i32,
    pub author_user_id: Uuid,
    pub author: String,
    pub parent_comment_id: Option<Uuid>,
    pub body: String,
    pub timecode_ms: Option<i64>,
    pub frame_number: Option<i32>,
    pub annotation: Option<Value>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewProxyResponse {
    pub path: String,
    pub revision_number: i32,
    pub blob_hash: String,
    pub size_bytes: i64,
    pub content_type: String,
    pub frame_rate_numerator: Option<i32>,
    pub frame_rate_denominator: Option<i32>,
    pub start_frame: Option<i32>,
}

pub async fn list_comments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReviewTargetQuery>,
) -> AppResult<Json<Vec<ReviewCommentResponse>>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let path = normalize_depot_path(&query.path)?;
    let revision_id =
        revision_for_target(&state.db, workspace.stream_id, &path, query.revision_number).await?;

    let rows = sqlx::query(
        r#"
        SELECT arc.id, fr.depot_path, fr.revision_number, arc.author_user_id,
               COALESCE(u.display_name, u.username) AS author,
               arc.parent_comment_id, arc.body, arc.timecode_ms, arc.frame_number,
               arc.annotation, arc.resolved_at, arc.resolved_by, arc.created_at
        FROM asset_review_comments arc
        JOIN file_revisions fr ON fr.id = arc.revision_id
        JOIN users u ON u.id = arc.author_user_id
        WHERE arc.revision_id = $1
        ORDER BY arc.created_at ASC, arc.id ASC
        "#,
    )
    .bind(revision_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(comment_from_row).collect()))
}

pub async fn create_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateReviewCommentRequest>,
) -> AppResult<Json<ReviewCommentResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, request.workspace_id, &user).await?;
    let path = normalize_depot_path(&request.path)?;
    let body = request.body.trim();
    if body.is_empty() || body.chars().count() > 4_000 {
        return Err(AppError::bad_request(
            "review comment must contain between 1 and 4000 characters",
        ));
    }
    validate_marker(request.timecode_ms, request.frame_number)?;
    validate_annotation(request.annotation.as_ref())?;
    let revision_id = revision_for_target(
        &state.db,
        workspace.stream_id,
        &path,
        request.revision_number,
    )
    .await?;

    if let Some(parent_id) = request.parent_comment_id {
        let parent_revision: Option<Uuid> =
            sqlx::query_scalar("SELECT revision_id FROM asset_review_comments WHERE id = $1")
                .bind(parent_id)
                .fetch_optional(&state.db)
                .await?;
        if parent_revision != Some(revision_id) {
            return Err(AppError::bad_request(
                "reply target must belong to the same asset revision",
            ));
        }
    }

    // Without a key a retried post would leave two identical notes on the same
    // frame, which is the one review outcome an artist cannot undo themselves.
    let idempotency_request = serde_json::json!({
        "revision_id": revision_id,
        "parent_comment_id": request.parent_comment_id,
        "body": body,
        "timecode_ms": request.timecode_ms,
        "frame_number": request.frame_number,
    });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "asset_review_comment_create",
        &idempotency_request,
        || async {
            let comment_id: Uuid = sqlx::query_scalar(
                r#"
        INSERT INTO asset_review_comments
            (revision_id, author_user_id, parent_comment_id, body,
             timecode_ms, frame_number, annotation)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
            )
            .bind(revision_id)
            .bind(user.user_id)
            .bind(request.parent_comment_id)
            .bind(body)
            .bind(request.timecode_ms)
            .bind(request.frame_number)
            .bind(request.annotation)
            .fetch_one(&state.db)
            .await?;

            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(workspace.stream_id),
                    workspace_id: Some(workspace.id),
                    depot_path: Some(&path),
                    ..audit::AuditEvent::new(
                        "asset_review_comment_create",
                        serde_json::json!({
                            "comment_id": comment_id,
                            "revision_number": request.revision_number,
                            "timecode_ms": request.timecode_ms,
                            "frame_number": request.frame_number,
                        }),
                    )
                },
            )
            .await?;

            comment_by_id(&state.db, comment_id).await
        },
    )
    .await?;
    Ok(Json(response))
}

pub async fn resolve_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(comment_id): AxumPath<Uuid>,
    Json(request): Json<ResolveReviewCommentRequest>,
) -> AppResult<Json<ReviewCommentResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, request.workspace_id, &user).await?;
    let row = sqlx::query(
        r#"
        SELECT fr.stream_id, fr.depot_path, fr.revision_number
        FROM asset_review_comments arc
        JOIN file_revisions fr ON fr.id = arc.revision_id
        WHERE arc.id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("review comment not found".to_string()))?;
    let stream_id: Uuid = row.get("stream_id");
    if stream_id != workspace.stream_id {
        return Err(AppError::Forbidden(
            "review comment is outside this workspace stream".to_string(),
        ));
    }

    let path: String = row.get("depot_path");
    let revision_number: i32 = row.get("revision_number");
    // The comment is addressed by path, so it has to be part of the hashed
    // request; otherwise one key would cover resolving any comment.
    let keyed_request = serde_json::json!({
        "comment_id": comment_id,
        "workspace_id": request.workspace_id,
        "resolved": request.resolved,
    });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "asset_review_comment_resolve",
        &keyed_request,
        || async {
            sqlx::query(
                r#"
        UPDATE asset_review_comments
        SET resolved_at = CASE WHEN $2 THEN now() ELSE NULL END,
            resolved_by = CASE WHEN $2 THEN $3 ELSE NULL END
        WHERE id = $1
        "#,
            )
            .bind(comment_id)
            .bind(request.resolved)
            .bind(user.user_id)
            .execute(&state.db)
            .await?;

            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(workspace.stream_id),
                    workspace_id: Some(workspace.id),
                    depot_path: Some(&path),
                    ..audit::AuditEvent::new(
                        "asset_review_comment_resolve",
                        serde_json::json!({
                            "comment_id": comment_id,
                            "revision_number": revision_number,
                            "resolved": request.resolved,
                        }),
                    )
                },
            )
            .await?;

            comment_by_id(&state.db, comment_id).await
        },
    )
    .await?;

    Ok(Json(response))
}

pub async fn download_review_media(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReviewTargetQuery>,
) -> AppResult<Response<Body>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let path = normalize_depot_path(&query.path)?;
    let row = sqlx::query(
        r#"
        SELECT blob_hash, size_bytes
        FROM file_revisions
        WHERE stream_id = $1 AND depot_path = $2 AND revision_number = $3
          AND action <> 'delete'
        "#,
    )
    .bind(workspace.stream_id)
    .bind(&path)
    .bind(query.revision_number)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("asset revision not found".to_string()))?;
    let blob_hash: String = row.get("blob_hash");
    let size_bytes: i64 = row.get("size_bytes");
    let content_type = review_content_type(&path);

    review_blob_response(&state, &headers, blob_hash, size_bytes, content_type)
}

pub async fn upload_review_proxy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReviewTargetQuery>,
    body: Body,
) -> AppResult<Json<ReviewProxyResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let path = normalize_depot_path(&query.path)?;
    let content_type = normalize_review_proxy_content_type(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
    )?;
    let timebase = validate_review_timebase(
        query.frame_rate_numerator,
        query.frame_rate_denominator,
        query.start_frame,
    )?;
    let revision_id =
        revision_for_target(&state.db, workspace.stream_id, &path, query.revision_number).await?;

    // Retained for older integrations. The body is streamed through bounded
    // object-store buffers; new clients should use upload sessions so a dropped
    // connection can resume instead of restarting.
    let _storage_lease = state.storage_maintenance.read().await;
    let reader = StreamReader::new(body.into_data_stream().map_err(std::io::Error::other));
    let manifest = state.storage.put_reader(reader).await?;
    if manifest.size_bytes == 0 {
        return Err(AppError::bad_request("review proxy cannot be empty"));
    }
    let response = persist_review_proxy(
        &state,
        &user,
        workspace.id,
        workspace.stream_id,
        &path,
        query.revision_number,
        revision_id,
        manifest,
        &content_type,
        timebase,
    )
    .await?;
    Ok(Json(response))
}

/// Attaches a finalized resumable upload to an immutable asset revision.
pub async fn attach_review_proxy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AttachReviewProxyRequest>,
) -> AppResult<Json<ReviewProxyResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, request.workspace_id, &user).await?;
    let path = normalize_depot_path(&request.path)?;
    let content_type = normalize_review_proxy_content_type(&request.content_type)?;
    let timebase = validate_review_timebase(
        request.frame_rate_numerator,
        request.frame_rate_denominator,
        request.start_frame,
    )?;
    let revision_id = revision_for_target(
        &state.db,
        workspace.stream_id,
        &path,
        request.revision_number,
    )
    .await?;
    let finalized = finalized_upload_for_target(
        &state,
        &user,
        request.upload_id,
        workspace.id,
        &path,
        REVIEW_PROXY_PURPOSE,
    )
    .await?;
    // Keep local orphan cleanup from removing finalized-but-not-yet-referenced
    // chunks while the proxy's blob records and immutable revision link commit.
    let _storage_lease = state.storage_maintenance.read().await;
    let manifest = state.storage.read_manifest(&finalized.blob_hash).await?;
    if manifest.size_bytes as i64 != finalized.size_bytes {
        return Err(AppError::conflict(
            "finalized upload size does not match its stored manifest",
        ));
    }
    let response = persist_review_proxy(
        &state,
        &user,
        workspace.id,
        workspace.stream_id,
        &path,
        request.revision_number,
        revision_id,
        manifest,
        &content_type,
        timebase,
    )
    .await?;
    Ok(Json(response))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReviewTimebase {
    frame_rate_numerator: i32,
    frame_rate_denominator: i32,
    start_frame: i32,
}

#[allow(clippy::too_many_arguments)]
async fn persist_review_proxy(
    state: &AppState,
    user: &crate::auth::AuthUser,
    workspace_id: Uuid,
    stream_id: Uuid,
    path: &str,
    revision_number: i32,
    revision_id: Uuid,
    manifest: crate::storage::BlobManifest,
    content_type: &str,
    timebase: Option<ReviewTimebase>,
) -> AppResult<ReviewProxyResponse> {
    let mut tx = state.db.begin().await?;
    insert_blob_records(&mut tx, &manifest).await?;
    let inserted: Option<Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO revision_review_proxies
            (revision_id, blob_hash, size_bytes, content_type, created_by,
             frame_rate_numerator, frame_rate_denominator, start_frame)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (revision_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(revision_id)
    .bind(&manifest.hash)
    .bind(manifest.size_bytes as i64)
    .bind(content_type)
    .bind(user.user_id)
    .bind(timebase.map(|value| value.frame_rate_numerator))
    .bind(timebase.map(|value| value.frame_rate_denominator))
    .bind(timebase.map(|value| value.start_frame))
    .fetch_optional(&mut *tx)
    .await?;
    if inserted.is_none() {
        let existing = sqlx::query(
            r#"
            SELECT blob_hash, content_type, frame_rate_numerator,
                   frame_rate_denominator, start_frame
            FROM revision_review_proxies
            WHERE revision_id = $1
            "#,
        )
        .bind(revision_id)
        .fetch_one(&mut *tx)
        .await?;
        let existing_timebase = match (
            existing.get::<Option<i32>, _>("frame_rate_numerator"),
            existing.get::<Option<i32>, _>("frame_rate_denominator"),
            existing.get::<Option<i32>, _>("start_frame"),
        ) {
            (Some(numerator), Some(denominator), Some(start_frame)) => Some(ReviewTimebase {
                frame_rate_numerator: numerator,
                frame_rate_denominator: denominator,
                start_frame,
            }),
            _ => None,
        };
        if existing.get::<String, _>("blob_hash") != manifest.hash
            || existing.get::<String, _>("content_type") != content_type
            || existing_timebase != timebase
        {
            return Err(AppError::conflict(
                "this immutable revision already has a different review proxy",
            ));
        }
    }
    tx.commit().await?;

    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(stream_id),
            workspace_id: Some(workspace_id),
            depot_path: Some(path),
            ..audit::AuditEvent::new(
                "review_proxy_upload",
                serde_json::json!({
                    "revision_number": revision_number,
                    "blob_hash": manifest.hash,
                    "content_type": content_type,
                    "frame_rate_numerator": timebase.map(|value| value.frame_rate_numerator),
                    "frame_rate_denominator": timebase.map(|value| value.frame_rate_denominator),
                    "start_frame": timebase.map(|value| value.start_frame),
                }),
            )
        },
    )
    .await?;

    Ok(ReviewProxyResponse {
        path: path.to_string(),
        revision_number,
        blob_hash: manifest.hash,
        size_bytes: manifest.size_bytes as i64,
        content_type: content_type.to_string(),
        frame_rate_numerator: timebase.map(|value| value.frame_rate_numerator),
        frame_rate_denominator: timebase.map(|value| value.frame_rate_denominator),
        start_frame: timebase.map(|value| value.start_frame),
    })
}

pub async fn download_review_proxy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReviewTargetQuery>,
) -> AppResult<Response<Body>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let path = normalize_depot_path(&query.path)?;
    let row = sqlx::query(
        r#"
        SELECT rrp.blob_hash, rrp.size_bytes, rrp.content_type,
               rrp.frame_rate_numerator, rrp.frame_rate_denominator,
               rrp.start_frame
        FROM file_revisions fr
        JOIN revision_review_proxies rrp ON rrp.revision_id = fr.id
        WHERE fr.stream_id = $1 AND fr.depot_path = $2 AND fr.revision_number = $3
        "#,
    )
    .bind(workspace.stream_id)
    .bind(&path)
    .bind(query.revision_number)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("asset review proxy not found".to_string()))?;
    let blob_hash: String = row.get("blob_hash");
    let size_bytes: i64 = row.get("size_bytes");
    let content_type: String = row.get("content_type");
    let frame_rate_numerator: Option<i32> = row.get("frame_rate_numerator");
    let frame_rate_denominator: Option<i32> = row.get("frame_rate_denominator");
    let start_frame: Option<i32> = row.get("start_frame");

    let mut response =
        review_blob_response(&state, &headers, blob_hash, size_bytes, &content_type)?;
    if let (Some(numerator), Some(denominator), Some(start_frame)) =
        (frame_rate_numerator, frame_rate_denominator, start_frame)
    {
        insert_numeric_header(&mut response, "x-review-frame-rate-numerator", numerator)?;
        insert_numeric_header(
            &mut response,
            "x-review-frame-rate-denominator",
            denominator,
        )?;
        insert_numeric_header(&mut response, "x-review-start-frame", start_frame)?;
    }
    Ok(response)
}

fn insert_numeric_header(
    response: &mut Response<Body>,
    name: &'static str,
    value: i32,
) -> AppResult<()> {
    response.headers_mut().insert(
        name,
        HeaderValue::from_str(&value.to_string())
            .map_err(|_| AppError::internal("invalid review metadata header"))?,
    );
    Ok(())
}

fn review_blob_response(
    state: &AppState,
    request_headers: &HeaderMap,
    blob_hash: String,
    size_bytes: i64,
    content_type: &str,
) -> AppResult<Response<Body>> {
    let total_size =
        u64::try_from(size_bytes).map_err(|_| AppError::internal("invalid review media size"))?;
    let range = requested_byte_range(request_headers, total_size)?;
    let (body, content_length, content_range) = match range {
        Some((start, end)) => (
            Body::from_stream(state.storage.stream_blob_range(blob_hash, start, end)),
            end - start + 1,
            Some(format!("bytes {start}-{end}/{total_size}")),
        ),
        None => (
            Body::from_stream(state.storage.stream_blob(blob_hash)),
            total_size,
            None,
        ),
    };
    let mut response = Response::new(body);
    if content_range.is_some() {
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
    }
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .map_err(|_| AppError::internal("invalid review media content type"))?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string())
            .map_err(|_| AppError::internal("invalid review media size"))?,
    );
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Some(content_range) = content_range {
        response.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&content_range)
                .map_err(|_| AppError::internal("invalid review media range"))?,
        );
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

fn requested_byte_range(headers: &HeaderMap, total_size: u64) -> AppResult<Option<(u64, u64)>> {
    let Some(value) = headers.get(header::RANGE) else {
        return Ok(None);
    };
    if total_size == 0 {
        return Err(AppError::bad_request(
            "empty review media has no byte ranges",
        ));
    }
    let value = value
        .to_str()
        .map_err(|_| AppError::bad_request("invalid Range header"))?;
    let value = value
        .strip_prefix("bytes=")
        .ok_or_else(|| AppError::bad_request("only byte ranges are supported"))?;
    if value.contains(',') {
        return Err(AppError::bad_request(
            "multiple byte ranges are not supported",
        ));
    }
    let (start, end) = value
        .split_once('-')
        .ok_or_else(|| AppError::bad_request("invalid byte range"))?;
    let (start, end) = if start.is_empty() {
        let suffix: u64 = end
            .parse()
            .map_err(|_| AppError::bad_request("invalid byte range suffix"))?;
        if suffix == 0 {
            return Err(AppError::bad_request("byte range suffix must be positive"));
        }
        (
            total_size.saturating_sub(suffix.min(total_size)),
            total_size - 1,
        )
    } else {
        let start: u64 = start
            .parse()
            .map_err(|_| AppError::bad_request("invalid byte range start"))?;
        let end = if end.is_empty() {
            total_size - 1
        } else {
            end.parse()
                .map_err(|_| AppError::bad_request("invalid byte range end"))?
        };
        (start, end.min(total_size - 1))
    };
    if start >= total_size || end < start {
        return Err(AppError::bad_request("byte range is outside the asset"));
    }
    Ok(Some((start, end)))
}

pub(crate) async fn revision_for_target(
    db: &PgPool,
    stream_id: Uuid,
    path: &str,
    revision_number: i32,
) -> AppResult<Uuid> {
    if revision_number <= 0 {
        return Err(AppError::bad_request("revision_number must be positive"));
    }
    sqlx::query_scalar(
        r#"
        SELECT id FROM file_revisions
        WHERE stream_id = $1 AND depot_path = $2 AND revision_number = $3
          AND action <> 'delete'
        "#,
    )
    .bind(stream_id)
    .bind(path)
    .bind(revision_number)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::NotFound("asset revision not found".to_string()))
}

async fn comment_by_id(db: &PgPool, comment_id: Uuid) -> AppResult<ReviewCommentResponse> {
    let row = sqlx::query(
        r#"
        SELECT arc.id, fr.depot_path, fr.revision_number, arc.author_user_id,
               COALESCE(u.display_name, u.username) AS author,
               arc.parent_comment_id, arc.body, arc.timecode_ms, arc.frame_number,
               arc.annotation, arc.resolved_at, arc.resolved_by, arc.created_at
        FROM asset_review_comments arc
        JOIN file_revisions fr ON fr.id = arc.revision_id
        JOIN users u ON u.id = arc.author_user_id
        WHERE arc.id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::NotFound("review comment not found".to_string()))?;
    Ok(comment_from_row(row))
}

fn comment_from_row(row: sqlx::postgres::PgRow) -> ReviewCommentResponse {
    ReviewCommentResponse {
        id: row.get("id"),
        path: row.get("depot_path"),
        revision_number: row.get("revision_number"),
        author_user_id: row.get("author_user_id"),
        author: row.get("author"),
        parent_comment_id: row.get("parent_comment_id"),
        body: row.get("body"),
        timecode_ms: row.get("timecode_ms"),
        frame_number: row.get("frame_number"),
        annotation: row.get("annotation"),
        resolved_at: row.get("resolved_at"),
        resolved_by: row.get("resolved_by"),
        created_at: row.get("created_at"),
    }
}

fn validate_marker(timecode_ms: Option<i64>, frame_number: Option<i32>) -> AppResult<()> {
    if timecode_ms.is_some_and(|value| value < 0) {
        return Err(AppError::bad_request("timecode_ms cannot be negative"));
    }
    if frame_number.is_some_and(|value| value < 0) {
        return Err(AppError::bad_request("frame_number cannot be negative"));
    }
    Ok(())
}

fn normalize_review_proxy_content_type(value: &str) -> AppResult<String> {
    let normalized = value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "model/gltf-binary"
            | "model/gltf+json"
            | "application/vnd.autodesk.fbx"
            | "video/mp4"
            | "video/webm"
    ) {
        return Ok(normalized);
    }
    Err(AppError::bad_request(
        "review proxy must be GLB, glTF, FBX, MP4, or WebM",
    ))
}

fn validate_review_timebase(
    numerator: Option<i32>,
    denominator: Option<i32>,
    start_frame: Option<i32>,
) -> AppResult<Option<ReviewTimebase>> {
    match (numerator, denominator, start_frame) {
        (None, None, None) => Ok(None),
        (Some(numerator), Some(denominator), start_frame) => {
            let start_frame = start_frame.unwrap_or(0);
            if !(1..=MAX_FRAME_RATE_NUMERATOR).contains(&numerator)
                || !(1..=MAX_FRAME_RATE_DENOMINATOR).contains(&denominator)
                || start_frame < 0
            {
                return Err(AppError::bad_request(
                    "review timebase must use positive bounded frame-rate values and a non-negative start frame",
                ));
            }
            let fps = f64::from(numerator) / f64::from(denominator);
            if !(0.1..=1_000.0).contains(&fps) {
                return Err(AppError::bad_request(
                    "review frame rate must be between 0.1 and 1000 fps",
                ));
            }
            Ok(Some(ReviewTimebase {
                frame_rate_numerator: numerator,
                frame_rate_denominator: denominator,
                start_frame,
            }))
        }
        _ => Err(AppError::bad_request(
            "frame_rate_numerator and frame_rate_denominator must be provided together",
        )),
    }
}

fn validate_annotation(annotation: Option<&Value>) -> AppResult<()> {
    let Some(annotation) = annotation else {
        return Ok(());
    };
    if !annotation.is_object() {
        return Err(AppError::bad_request("annotation must be a JSON object"));
    }
    if serde_json::to_vec(annotation)?.len() > MAX_ANNOTATION_BYTES {
        return Err(AppError::bad_request("annotation exceeds 128 KiB"));
    }
    Ok(())
}

fn review_content_type(path: &str) -> &'static str {
    let extension = path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "fbx" => "application/vnd.autodesk.fbx",
        _ => "application/octet-stream",
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use axum::http::{header, HeaderMap, HeaderValue};
    use serde_json::json;

    use super::{
        normalize_review_proxy_content_type, requested_byte_range, review_content_type,
        validate_annotation, validate_marker, validate_review_timebase, ReviewTimebase,
    };

    #[test]
    fn maps_reviewable_media_types_without_exposing_active_content() {
        assert_eq!(review_content_type("Models/Hero.glb"), "model/gltf-binary");
        assert_eq!(review_content_type("Playblast/shot.webm"), "video/webm");
        assert_eq!(
            review_content_type("Docs/page.html"),
            "application/octet-stream"
        );
    }

    #[test]
    fn validates_review_markers_and_annotation_shape() {
        assert!(validate_marker(Some(1_250), Some(30)).is_ok());
        assert!(validate_marker(Some(-1), None).is_err());
        assert!(validate_annotation(Some(&json!({"tool": "pen", "points": []}))).is_ok());
        assert!(validate_annotation(Some(&json!([1, 2, 3]))).is_err());
    }

    #[test]
    fn validates_exact_review_timebases_and_proxy_types() {
        assert_eq!(
            validate_review_timebase(Some(24_000), Some(1_001), Some(1_001)).unwrap(),
            Some(ReviewTimebase {
                frame_rate_numerator: 24_000,
                frame_rate_denominator: 1_001,
                start_frame: 1_001,
            })
        );
        assert!(validate_review_timebase(Some(24), None, None).is_err());
        assert!(validate_review_timebase(Some(0), Some(1), None).is_err());
        assert_eq!(
            normalize_review_proxy_content_type("model/gltf-binary; charset=binary").unwrap(),
            "model/gltf-binary"
        );
        assert!(normalize_review_proxy_content_type("text/html").is_err());
    }

    #[test]
    fn parses_browser_media_ranges() {
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=100-199"));
        assert_eq!(
            requested_byte_range(&headers, 1_000).unwrap(),
            Some((100, 199))
        );
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=-50"));
        assert_eq!(
            requested_byte_range(&headers, 1_000).unwrap(),
            Some((950, 999))
        );
    }
}
