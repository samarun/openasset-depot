use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    auth::AuthUser,
    depot::{resolve_depot_id, validate_name},
    error::{AppError, AppResult},
    idempotency,
    permissions::{ensure_depot_permission, DepotPermission},
    streams::resolve_stream_id,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub depot: String,
    pub stream: String,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub depot_id: Uuid,
    pub stream_id: Uuid,
    pub name: String,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteWorkspaceResponse {
    pub id: Uuid,
    pub deleted: bool,
    pub released_locks: u64,
    pub abandoned_changelists: u64,
}

#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    pub id: Uuid,
    pub user_id: Uuid,
    pub depot_id: Uuid,
    pub stream_id: Uuid,
}

pub async fn create_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateWorkspaceRequest>,
) -> AppResult<Json<WorkspaceResponse>> {
    let user = state.require_user(&headers)?;
    validate_name(&req.name)?;
    let depot_id = resolve_depot_id(&state.db, &req.depot).await?;
    ensure_depot_permission(&state.db, &user, depot_id, DepotPermission::Read).await?;
    let stream_id = resolve_stream_id(&state.db, depot_id, &req.stream).await?;
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "workspace_create",
        &req,
        || async {
            let row = sqlx::query(
                r#"
                INSERT INTO workspaces (user_id, depot_id, stream_id, name, local_path)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id, user_id, depot_id, stream_id, name, local_path
                "#,
            )
            .bind(user.user_id)
            .bind(depot_id)
            .bind(stream_id)
            .bind(&req.name)
            .bind(&req.local_path)
            .fetch_one(&state.db)
            .await
            .map_err(map_unique_conflict("workspace already exists for user"))?;
            let response = workspace_response(row);
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(stream_id),
                    workspace_id: Some(response.id),
                    ..audit::AuditEvent::new(
                        "workspace_create",
                        serde_json::json!({ "name": response.name, "local_path": response.local_path }),
                    )
                },
            )
            .await?;
            Ok(response)
        },
    )
    .await?;
    Ok(Json(response))
}

pub async fn list_workspaces(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<WorkspaceResponse>>> {
    let user = state.require_user(&headers)?;
    let rows = sqlx::query(
        "SELECT id, user_id, depot_id, stream_id, name, local_path FROM workspaces WHERE user_id = $1 AND deleted_at IS NULL ORDER BY name",
    )
    .bind(user.user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(workspace_response).collect()))
}

pub async fn delete_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
) -> AppResult<Json<DeleteWorkspaceResponse>> {
    let user = state.require_user(&headers)?;
    let mut tx = state.db.begin().await?;
    let row = sqlx::query(
        r#"
        SELECT id, stream_id
        FROM workspaces
        WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(workspace_id)
    .bind(user.user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("workspace not found".to_string()))?;
    let stream_id: Uuid = row.get("stream_id");

    let released_locks = sqlx::query(
        r#"
        UPDATE locks
        SET state = 'released', released_at = now()
        WHERE workspace_id = $1 AND state = 'active'
        "#,
    )
    .bind(workspace_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let abandoned_changelists = sqlx::query(
        r#"
        UPDATE changelists
        SET status = 'abandoned'
        WHERE workspace_id = $1 AND status = 'pending'
        "#,
    )
    .bind(workspace_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    sqlx::query("UPDATE workspaces SET deleted_at = now(), updated_at = now() WHERE id = $1")
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;

    audit::record_tx(
        &mut tx,
        audit::AuditEvent {
            actor_user_id: Some(user.user_id),
            stream_id: Some(stream_id),
            workspace_id: Some(workspace_id),
            ..audit::AuditEvent::new(
                "workspace_delete",
                serde_json::json!({
                    "released_locks": released_locks,
                    "abandoned_changelists": abandoned_changelists,
                }),
            )
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(DeleteWorkspaceResponse {
        id: workspace_id,
        deleted: true,
        released_locks,
        abandoned_changelists,
    }))
}

pub async fn workspace_for_user(
    db: &sqlx::PgPool,
    workspace_id: Uuid,
    user: &AuthUser,
) -> AppResult<WorkspaceContext> {
    let row = sqlx::query(
        "SELECT id, user_id, depot_id, stream_id FROM workspaces WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(workspace_id)
    .bind(user.user_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::NotFound("workspace not found".to_string()))?;
    let context = WorkspaceContext {
        id: row.get("id"),
        user_id: row.get("user_id"),
        depot_id: row.get("depot_id"),
        stream_id: row.get("stream_id"),
    };
    ensure_depot_permission(db, user, context.depot_id, DepotPermission::Read).await?;
    Ok(context)
}

fn workspace_response(row: sqlx::postgres::PgRow) -> WorkspaceResponse {
    WorkspaceResponse {
        id: row.get("id"),
        user_id: row.get("user_id"),
        depot_id: row.get("depot_id"),
        stream_id: row.get("stream_id"),
        name: row.get("name"),
        local_path: row.get("local_path"),
    }
}

fn map_unique_conflict(message: &'static str) -> impl FnOnce(sqlx::Error) -> AppError {
    move |error| match &error {
        sqlx::Error::Database(db_error) if db_error.is_unique_violation() => {
            AppError::conflict(message)
        }
        _ => AppError::Database(error),
    }
}
