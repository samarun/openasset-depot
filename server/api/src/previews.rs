use std::io::Cursor;

use axum::{
    body::{Body, Bytes},
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    changelists::insert_blob_records,
    error::{AppError, AppResult},
    paths::normalize_depot_path,
    workspaces::workspace_for_user,
};

const MAX_PREVIEW_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    pub workspace_id: Uuid,
    pub path: String,
    pub revision_number: i32,
}

#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    pub path: String,
    pub revision_number: i32,
    pub blob_hash: String,
    pub size_bytes: i64,
    pub content_type: String,
}

pub async fn upload_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PreviewQuery>,
    body: Bytes,
) -> AppResult<Json<PreviewResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let path = normalize_depot_path(&query.path)?;
    if query.revision_number <= 0 {
        return Err(AppError::bad_request("revision_number must be positive"));
    }
    if body.is_empty() || body.len() > MAX_PREVIEW_BYTES {
        return Err(AppError::bad_request(
            "preview must contain between 1 byte and 8 MiB",
        ));
    }
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !matches!(
        content_type.as_str(),
        "image/png" | "image/jpeg" | "image/webp"
    ) {
        return Err(AppError::bad_request(
            "preview content type must be image/png, image/jpeg, or image/webp",
        ));
    }

    let revision = sqlx::query(
        r#"
        SELECT fr.id, fr.file_id
        FROM file_revisions fr
        WHERE fr.stream_id = $1
          AND fr.depot_path = $2
          AND fr.revision_number = $3
          AND fr.action <> 'delete'
        "#,
    )
    .bind(workspace.stream_id)
    .bind(&path)
    .bind(query.revision_number)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("asset revision not found".to_string()))?;
    let revision_id: Uuid = revision.get("id");

    let _storage_lease = state.storage_maintenance.read().await;
    let manifest = state.storage.put_reader(Cursor::new(body.to_vec())).await?;
    let mut tx = state.db.begin().await?;
    insert_blob_records(&mut tx, &manifest).await?;
    let inserted: Option<Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO revision_previews
            (revision_id, blob_hash, size_bytes, content_type, created_by)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (revision_id) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(revision_id)
    .bind(&manifest.hash)
    .bind(manifest.size_bytes as i64)
    .bind(&content_type)
    .bind(user.user_id)
    .fetch_optional(&mut *tx)
    .await?;
    if inserted.is_none() {
        let existing_hash: String =
            sqlx::query_scalar("SELECT blob_hash FROM revision_previews WHERE revision_id = $1")
                .bind(revision_id)
                .fetch_one(&mut *tx)
                .await?;
        if existing_hash != manifest.hash {
            return Err(AppError::conflict(
                "this immutable revision already has a different preview",
            ));
        }
    }
    tx.commit().await?;

    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(workspace.stream_id),
            workspace_id: Some(workspace.id),
            depot_path: Some(&path),
            ..audit::AuditEvent::new(
                "preview_upload",
                serde_json::json!({
                    "revision_number": query.revision_number,
                    "blob_hash": manifest.hash,
                }),
            )
        },
    )
    .await?;

    Ok(Json(PreviewResponse {
        path,
        revision_number: query.revision_number,
        blob_hash: manifest.hash,
        size_bytes: manifest.size_bytes as i64,
        content_type,
    }))
}

pub async fn download_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PreviewQuery>,
) -> AppResult<Response<Body>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let path = normalize_depot_path(&query.path)?;
    let row = sqlx::query(
        r#"
        SELECT rp.blob_hash, rp.content_type
        FROM file_revisions fr
        JOIN revision_previews rp ON rp.revision_id = fr.id
        WHERE fr.stream_id = $1 AND fr.depot_path = $2 AND fr.revision_number = $3
        "#,
    )
    .bind(workspace.stream_id)
    .bind(&path)
    .bind(query.revision_number)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("asset preview not found".to_string()))?;
    let blob_hash: String = row.get("blob_hash");
    let content_type: String = row.get("content_type");

    let mut response = Response::new(Body::from_stream(state.storage.stream_blob(blob_hash)));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .map_err(|_| AppError::internal("invalid preview content type"))?,
    );
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
