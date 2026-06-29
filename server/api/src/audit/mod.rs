use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    api::AppState,
    error::{AppError, AppResult},
};

#[derive(Debug)]
pub struct AuditEvent<'a> {
    pub event_type: &'a str,
    pub actor_user_id: Option<Uuid>,
    pub stream_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub depot_path: Option<&'a str>,
    pub changelist_id: Option<Uuid>,
    pub details: Value,
}

impl<'a> AuditEvent<'a> {
    pub fn new(event_type: &'a str, details: Value) -> Self {
        Self {
            event_type,
            actor_user_id: None,
            stream_id: None,
            workspace_id: None,
            depot_path: None,
            changelist_id: None,
            details,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuditPageQuery {
    pub limit: Option<i64>,
    pub before_created_at: Option<DateTime<Utc>>,
    pub before_id: Option<Uuid>,
    pub event_type: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub stream_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct AuditEventResponse {
    pub id: Uuid,
    pub event_type: String,
    pub actor_user_id: Option<Uuid>,
    pub stream_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub depot_path: Option<String>,
    pub changelist_id: Option<Uuid>,
    pub details: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AuditPageResponse {
    pub items: Vec<AuditEventResponse>,
    pub next_before_created_at: Option<DateTime<Utc>>,
    pub next_before_id: Option<Uuid>,
}

pub async fn list_audit_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuditPageQuery>,
) -> AppResult<Json<AuditPageResponse>> {
    let user = state.require_user(&headers)?;
    if !user.is_admin {
        return Err(AppError::Forbidden(
            "only system admins can read the audit log".to_string(),
        ));
    }
    if query.before_created_at.is_some() != query.before_id.is_some() {
        return Err(AppError::bad_request(
            "before_created_at and before_id must be supplied together",
        ));
    }
    let limit = query.limit.unwrap_or(250).clamp(1, 1_000) as usize;
    let rows = sqlx::query(
        r#"
        SELECT id, event_type, actor_user_id, stream_id, workspace_id,
               depot_path, changelist_id, details, created_at
        FROM audit_events
        WHERE ($1::text IS NULL OR event_type = $1)
          AND ($2::uuid IS NULL OR actor_user_id = $2)
          AND ($3::uuid IS NULL OR stream_id = $3)
          AND ($4::uuid IS NULL OR workspace_id = $4)
          AND ($5::timestamptz IS NULL OR (created_at, id) < ($5, $6::uuid))
        ORDER BY created_at DESC, id DESC
        LIMIT $7
        "#,
    )
    .bind(query.event_type)
    .bind(query.actor_user_id)
    .bind(query.stream_id)
    .bind(query.workspace_id)
    .bind(query.before_created_at)
    .bind(query.before_id)
    .bind((limit + 1) as i64)
    .fetch_all(&state.db)
    .await?;

    let mut items: Vec<_> = rows
        .into_iter()
        .map(|row| AuditEventResponse {
            id: row.get("id"),
            event_type: row.get("event_type"),
            actor_user_id: row.get("actor_user_id"),
            stream_id: row.get("stream_id"),
            workspace_id: row.get("workspace_id"),
            depot_path: row.get("depot_path"),
            changelist_id: row.get("changelist_id"),
            details: row.get("details"),
            created_at: row.get("created_at"),
        })
        .collect();
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

    Ok(Json(AuditPageResponse {
        items,
        next_before_created_at,
        next_before_id,
    }))
}

pub async fn record(db: &PgPool, event: AuditEvent<'_>) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_events
            (event_type, actor_user_id, stream_id, workspace_id, depot_path, changelist_id, details)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(event.event_type)
    .bind(event.actor_user_id)
    .bind(event.stream_id)
    .bind(event.workspace_id)
    .bind(event.depot_path)
    .bind(event.changelist_id)
    .bind(event.details)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn record_tx(tx: &mut Transaction<'_, Postgres>, event: AuditEvent<'_>) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_events
            (event_type, actor_user_id, stream_id, workspace_id, depot_path, changelist_id, details)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(event.event_type)
    .bind(event.actor_user_id)
    .bind(event.stream_id)
    .bind(event.workspace_id)
    .bind(event.depot_path)
    .bind(event.changelist_id)
    .bind(event.details)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
