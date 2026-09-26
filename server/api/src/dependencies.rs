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
    workspaces::workspace_for_user,
};

/// Cap on each side of an impact report.
///
/// The point of the inspector panel is to answer "is this risky to change" at a
/// glance. Past a few dozen entries the honest answer is "yes, a lot", and the
/// `truncated` flag says so without shipping thousands of rows to a side panel.
const MAX_IMPACT_ROWS: i64 = 50;

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

#[derive(Debug, Deserialize)]
pub struct DependencyImpactQuery {
    pub workspace_id: Uuid,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct DependencyImpactEdge {
    /// The other file in the relationship, whichever direction was asked for.
    pub path: String,
    pub dependency_type: String,
    pub dependency_status: String,
    pub adapter_name: String,
    pub confidence: f64,
    pub scan_time: DateTime<Utc>,
    /// Whether that path currently exists in the depot, undeleted.
    pub in_depot: bool,
}

#[derive(Debug, Serialize)]
pub struct DependencyImpactResponse {
    pub path: String,
    /// Files that reference this one, so changing it can break them.
    pub required_by: Vec<DependencyImpactEdge>,
    /// Files this one references and therefore needs present to open cleanly.
    pub depends_on: Vec<DependencyImpactEdge>,
    pub required_by_count: i64,
    pub depends_on_count: i64,
    /// References this file makes that no adapter could resolve.
    pub missing_count: i64,
    /// True when either list was cut off at the reporting cap.
    pub truncated: bool,
    /// When the graph last saw this file. `None` means it was never scanned, so
    /// empty lists mean "unknown" rather than "nothing depends on this".
    pub last_scanned_at: Option<DateTime<Utc>>,
}

/// Reports both directions of the dependency graph around one file.
///
/// The reverse direction is the one that changes behaviour: it tells an artist
/// how many other assets reference the file they are about to check out. Rows
/// are deduplicated by the other path, because several adapters scanning the
/// same scene each report the same edge and a count inflated by adapter count
/// would be misleading.
pub async fn dependency_impact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DependencyImpactQuery>,
) -> AppResult<Json<DependencyImpactResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let path = normalize_depot_path(&query.path)?;

    let required_by = impact_edges(
        &state.db,
        workspace.stream_id,
        &path,
        ImpactDirection::RequiredBy,
    )
    .await?;
    let depends_on = impact_edges(
        &state.db,
        workspace.stream_id,
        &path,
        ImpactDirection::DependsOn,
    )
    .await?;

    let totals = sqlx::query(
        r#"
        SELECT
            count(DISTINCT source_file) FILTER (WHERE target_file = $2) AS required_by_count,
            count(DISTINCT target_file) FILTER (WHERE source_file = $2) AS depends_on_count,
            count(DISTINCT target_file) FILTER (
                WHERE source_file = $2 AND dependency_status = 'missing'
            ) AS missing_count,
            max(scan_time) FILTER (WHERE source_file = $2 OR target_file = $2) AS last_scanned_at
        FROM asset_dependency_edges
        WHERE stream_id = $1 AND (source_file = $2 OR target_file = $2)
        "#,
    )
    .bind(workspace.stream_id)
    .bind(&path)
    .fetch_one(&state.db)
    .await?;

    let required_by_count: i64 = totals.get("required_by_count");
    let depends_on_count: i64 = totals.get("depends_on_count");
    Ok(Json(DependencyImpactResponse {
        path,
        truncated: required_by_count > required_by.len() as i64
            || depends_on_count > depends_on.len() as i64,
        required_by,
        depends_on,
        required_by_count,
        depends_on_count,
        missing_count: totals.get("missing_count"),
        last_scanned_at: totals.get("last_scanned_at"),
    }))
}

enum ImpactDirection {
    /// Rows where the subject is the target: other files pointing at it.
    RequiredBy,
    /// Rows where the subject is the source: what it points at.
    DependsOn,
}

async fn impact_edges(
    db: &sqlx::PgPool,
    stream_id: Uuid,
    path: &str,
    direction: ImpactDirection,
) -> AppResult<Vec<DependencyImpactEdge>> {
    // `DISTINCT ON` keeps the most recent scan per related path, so a stale edge
    // from an older adapter run cannot mask the current one.
    let sql = match direction {
        ImpactDirection::RequiredBy => {
            r#"
            SELECT DISTINCT ON (e.source_file)
                   e.source_file AS other_path, e.dependency_type, e.dependency_status,
                   e.adapter_name, e.confidence::double precision AS confidence, e.scan_time,
                   EXISTS (
                       SELECT 1 FROM files f
                       WHERE f.stream_id = e.stream_id AND f.depot_path = e.source_file
                         AND f.deleted = FALSE
                   ) AS in_depot
            FROM asset_dependency_edges e
            WHERE e.stream_id = $1 AND e.target_file = $2
            ORDER BY e.source_file, e.scan_time DESC, e.id DESC
            LIMIT $3
            "#
        }
        ImpactDirection::DependsOn => {
            r#"
            SELECT DISTINCT ON (e.target_file)
                   e.target_file AS other_path, e.dependency_type, e.dependency_status,
                   e.adapter_name, e.confidence::double precision AS confidence, e.scan_time,
                   EXISTS (
                       SELECT 1 FROM files f
                       WHERE f.stream_id = e.stream_id AND f.depot_path = e.target_file
                         AND f.deleted = FALSE
                   ) AS in_depot
            FROM asset_dependency_edges e
            WHERE e.stream_id = $1 AND e.source_file = $2
            ORDER BY e.target_file, e.scan_time DESC, e.id DESC
            LIMIT $3
            "#
        }
    };

    let rows = sqlx::query(sql)
        .bind(stream_id)
        .bind(path)
        .bind(MAX_IMPACT_ROWS)
        .fetch_all(db)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| DependencyImpactEdge {
            path: row.get("other_path"),
            dependency_type: row.get("dependency_type"),
            dependency_status: row.get("dependency_status"),
            adapter_name: row.get("adapter_name"),
            confidence: row.get("confidence"),
            scan_time: row.get("scan_time"),
            in_depot: row.get("in_depot"),
        })
        .collect())
}
