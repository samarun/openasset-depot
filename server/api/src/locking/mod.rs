use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    error::{AppError, AppResult},
    paths::normalize_depot_path,
    workspaces::workspace_for_user,
};

#[derive(Debug, Deserialize)]
pub struct LockRequest {
    pub workspace_id: Uuid,
    pub path: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UnlockRequest {
    pub workspace_id: Uuid,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct LockResponse {
    pub id: Uuid,
    pub stream_id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub depot_path: String,
    pub reason: Option<String>,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LockPageQuery {
    pub limit: Option<i64>,
    pub before_created_at: Option<DateTime<Utc>>,
    pub before_id: Option<Uuid>,
    pub stream_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct LockPageResponse {
    pub items: Vec<LockResponse>,
    pub next_before_created_at: Option<DateTime<Utc>>,
    pub next_before_id: Option<Uuid>,
}

pub async fn lock_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LockRequest>,
) -> AppResult<Json<LockResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let depot_path = normalize_depot_path(&req.path)?;

    let mut tx = state.db.begin().await?;
    let row = sqlx::query(
        r#"
        INSERT INTO locks (stream_id, workspace_id, user_id, depot_path, reason)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, stream_id, workspace_id, user_id, depot_path, reason, state, created_at
        "#,
    )
    .bind(workspace.stream_id)
    .bind(workspace.id)
    .bind(user.user_id)
    .bind(&depot_path)
    .bind(req.reason)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_lock_conflict)?;

    audit::record_tx(
        &mut tx,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(workspace.stream_id),
            workspace_id: Some(workspace.id),
            depot_path: Some(&depot_path),
            ..audit::AuditEvent::new("file_lock", serde_json::json!({}))
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(lock_response(row)))
}

pub async fn unlock_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UnlockRequest>,
) -> AppResult<Json<LockResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, req.workspace_id, &user).await?;
    let depot_path = normalize_depot_path(&req.path)?;
    let mut tx = state.db.begin().await?;

    let row = sqlx::query(
        r#"
        UPDATE locks
        SET state = 'released', released_at = now()
        WHERE stream_id = $1 AND workspace_id = $2 AND user_id = $3 AND depot_path = $4 AND state = 'active'
        RETURNING id, stream_id, workspace_id, user_id, depot_path, reason, state, created_at
        "#,
    )
    .bind(workspace.stream_id)
    .bind(workspace.id)
    .bind(user.user_id)
    .bind(&depot_path)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("active lock not found for user/workspace/path".to_string()))?;

    audit::record_tx(
        &mut tx,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(workspace.stream_id),
            workspace_id: Some(workspace.id),
            depot_path: Some(&depot_path),
            ..audit::AuditEvent::new("file_unlock", serde_json::json!({}))
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(lock_response(row)))
}

pub async fn list_locks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<LockResponse>>> {
    let user = state.require_user(&headers)?;
    let rows = if user.is_admin {
        sqlx::query(
            r#"
        SELECT id, stream_id, workspace_id, user_id, depot_path, reason, state, created_at
        FROM locks
        WHERE state = 'active'
        ORDER BY created_at DESC
        "#,
        )
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT DISTINCT
                l.id, l.stream_id, l.workspace_id, l.user_id,
                l.depot_path, l.reason, l.state, l.created_at
            FROM locks l
            JOIN streams s ON s.id = l.stream_id
            JOIN depots d ON d.id = s.depot_id
            LEFT JOIN depot_user_permissions p
              ON p.depot_id = d.id AND p.user_id = $1
            WHERE l.state = 'active'
              AND (d.owner_user_id = $1 OR p.role IN ('read', 'write', 'admin'))
            ORDER BY l.created_at DESC
            "#,
        )
        .bind(user.user_id)
        .fetch_all(&state.db)
        .await?
    };
    Ok(Json(rows.into_iter().map(lock_response).collect()))
}

pub async fn list_locks_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LockPageQuery>,
) -> AppResult<Json<LockPageResponse>> {
    let user = state.require_user(&headers)?;
    let limit = query.limit.unwrap_or(250).clamp(1, 1_000) as usize;
    let (before_created_at, before_id) = checked_cursor(
        query.before_created_at,
        query.before_id,
        "before_created_at and before_id must be supplied together",
    )?;

    let rows = if user.is_admin {
        sqlx::query(
            r#"
            SELECT id, stream_id, workspace_id, user_id, depot_path, reason, state, created_at
            FROM locks l
            WHERE l.state = 'active'
              AND ($1::uuid IS NULL OR l.stream_id = $1)
              AND ($2::uuid IS NULL OR l.workspace_id = $2)
              AND ($3::uuid IS NULL OR l.user_id = $3)
              AND ($4::timestamptz IS NULL OR (l.created_at, l.id) < ($4, $5::uuid))
            ORDER BY l.created_at DESC, l.id DESC
            LIMIT $6
            "#,
        )
        .bind(query.stream_id)
        .bind(query.workspace_id)
        .bind(query.user_id)
        .bind(before_created_at)
        .bind(before_id)
        .bind((limit + 1) as i64)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT
                l.id, l.stream_id, l.workspace_id, l.user_id,
                l.depot_path, l.reason, l.state, l.created_at
            FROM locks l
            JOIN streams s ON s.id = l.stream_id
            JOIN depots d ON d.id = s.depot_id
            LEFT JOIN depot_user_permissions p
              ON p.depot_id = d.id AND p.user_id = $1
            WHERE l.state = 'active'
              AND (d.owner_user_id = $1 OR p.role IN ('read', 'write', 'admin'))
              AND ($2::uuid IS NULL OR l.stream_id = $2)
              AND ($3::uuid IS NULL OR l.workspace_id = $3)
              AND ($4::uuid IS NULL OR l.user_id = $4)
              AND ($5::timestamptz IS NULL OR (l.created_at, l.id) < ($5, $6::uuid))
            ORDER BY l.created_at DESC, l.id DESC
            LIMIT $7
            "#,
        )
        .bind(user.user_id)
        .bind(query.stream_id)
        .bind(query.workspace_id)
        .bind(query.user_id)
        .bind(before_created_at)
        .bind(before_id)
        .bind((limit + 1) as i64)
        .fetch_all(&state.db)
        .await?
    };

    let mut items: Vec<_> = rows.into_iter().map(lock_response).collect();
    let has_more = items.len() > limit;
    items.truncate(limit);
    let (next_before_created_at, next_before_id) = if has_more {
        items
            .last()
            .map(|item| (Some(item.created_at), Some(item.id)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    Ok(Json(LockPageResponse {
        items,
        next_before_created_at,
        next_before_id,
    }))
}

fn checked_cursor(
    timestamp: Option<DateTime<Utc>>,
    id: Option<Uuid>,
    message: &'static str,
) -> AppResult<(Option<DateTime<Utc>>, Option<Uuid>)> {
    if timestamp.is_some() != id.is_some() {
        return Err(AppError::bad_request(message));
    }
    Ok((timestamp, id))
}

pub async fn user_holds_lock(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    stream_id: Uuid,
    workspace_id: Uuid,
    user_id: Uuid,
    depot_path: &str,
) -> AppResult<bool> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM locks
        WHERE stream_id = $1
          AND workspace_id = $2
          AND user_id = $3
          AND depot_path = $4
          AND state = 'active'
        "#,
    )
    .bind(stream_id)
    .bind(workspace_id)
    .bind(user_id)
    .bind(depot_path)
    .fetch_one(&mut **tx)
    .await?;
    Ok(count > 0)
}

fn lock_response(row: sqlx::postgres::PgRow) -> LockResponse {
    LockResponse {
        id: row.get("id"),
        stream_id: row.get("stream_id"),
        workspace_id: row.get("workspace_id"),
        user_id: row.get("user_id"),
        depot_path: row.get("depot_path"),
        reason: row.get("reason"),
        state: row.get("state"),
        created_at: row.get("created_at"),
    }
}

fn map_lock_conflict(error: sqlx::Error) -> AppError {
    match &error {
        sqlx::Error::Database(db_error) if db_error.is_unique_violation() => {
            AppError::conflict("file is already exclusively locked in this stream")
        }
        _ => AppError::Database(error),
    }
}
