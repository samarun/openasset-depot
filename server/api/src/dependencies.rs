use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    error::{AppError, AppResult},
    paths::normalize_depot_path,
    permissions::{ensure_depot_permission, DepotPermission},
};

#[derive(Debug, Deserialize)]
pub struct DependencyPageQuery {
    pub stream_id: Uuid,
    pub source_file: Option<String>,
    pub limit: Option<i64>,
    pub before_scan_time: Option<DateTime<Utc>>,
    pub before_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct DependencyEdgeResponse {
    pub id: Uuid,
    pub stream_id: Uuid,
    pub source_file: String,
    pub target_file: String,
    pub adapter_name: String,
    pub dependency_type: String,
    pub dependency_status: String,
    pub scan_time: DateTime<Utc>,
    pub confidence: f64,
    pub metadata: Value,
}

#[derive(Debug, Serialize)]
pub struct DependencyPageResponse {
    pub items: Vec<DependencyEdgeResponse>,
    pub next_before_scan_time: Option<DateTime<Utc>>,
    pub next_before_id: Option<Uuid>,
}

pub async fn list_dependencies(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DependencyPageQuery>,
) -> AppResult<Json<DependencyPageResponse>> {
    let user = state.require_user(&headers)?;
    if query.before_scan_time.is_some() != query.before_id.is_some() {
        return Err(AppError::bad_request(
            "before_scan_time and before_id must be supplied together",
        ));
    }
    let depot_id: Uuid = sqlx::query_scalar("SELECT depot_id FROM streams WHERE id = $1")
        .bind(query.stream_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("stream not found".to_string()))?;
    ensure_depot_permission(&state.db, &user, depot_id, DepotPermission::Read).await?;

    let source_file = query
        .source_file
        .map(|path| normalize_depot_path(&path))
        .transpose()?;
    let limit = query.limit.unwrap_or(250).clamp(1, 1_000) as usize;
    let rows = sqlx::query(
        r#"
        SELECT id, stream_id, source_file, target_file, adapter_name,
               dependency_type, dependency_status, scan_time,
               confidence::double precision AS confidence, metadata
        FROM asset_dependency_edges
        WHERE stream_id = $1
          AND ($2::text IS NULL OR source_file = $2)
          AND ($3::timestamptz IS NULL OR (scan_time, id) < ($3, $4::uuid))
        ORDER BY scan_time DESC, id DESC
        LIMIT $5
        "#,
    )
    .bind(query.stream_id)
    .bind(source_file)
    .bind(query.before_scan_time)
    .bind(query.before_id)
    .bind((limit + 1) as i64)
    .fetch_all(&state.db)
    .await?;

    let mut items: Vec<_> = rows
        .into_iter()
        .map(|row| DependencyEdgeResponse {
            id: row.get("id"),
            stream_id: row.get("stream_id"),
            source_file: row.get("source_file"),
            target_file: row.get("target_file"),
            adapter_name: row.get("adapter_name"),
            dependency_type: row.get("dependency_type"),
            dependency_status: row.get("dependency_status"),
            scan_time: row.get("scan_time"),
            confidence: row.get("confidence"),
            metadata: row.get("metadata"),
        })
        .collect();
    let has_more = items.len() > limit;
    items.truncate(limit);
    let (next_before_scan_time, next_before_id) = if has_more {
        items
            .last()
            .map(|item| (Some(item.scan_time), Some(item.id)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    Ok(Json(DependencyPageResponse {
        items,
        next_before_scan_time,
        next_before_id,
    }))
}
