use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    depot::{resolve_depot_id, validate_name},
    error::{AppError, AppResult},
    idempotency,
    permissions::{ensure_depot_permission, DepotPermission},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateStreamRequest {
    pub name: String,
    pub depot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamResponse {
    pub id: Uuid,
    pub depot_id: Uuid,
    pub name: String,
}

pub async fn create_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStreamRequest>,
) -> AppResult<Json<StreamResponse>> {
    let user = state.require_user(&headers)?;
    validate_name(&req.name)?;
    let depot_id = resolve_depot_id(&state.db, &req.depot).await?;
    ensure_depot_permission(&state.db, &user, depot_id, DepotPermission::Write).await?;
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "stream_create",
        &req,
        || async {
            let row = sqlx::query(
                r#"
                INSERT INTO streams (depot_id, name)
                VALUES ($1, $2)
                RETURNING id, depot_id, name
                "#,
            )
            .bind(depot_id)
            .bind(&req.name)
            .fetch_one(&state.db)
            .await
            .map_err(map_unique_conflict("stream already exists in depot"))?;
            let response = StreamResponse {
                id: row.get("id"),
                depot_id: row.get("depot_id"),
                name: row.get("name"),
            };
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(response.id),
                    ..audit::AuditEvent::new(
                        "stream_create",
                        serde_json::json!({ "depot_id": response.depot_id, "name": response.name }),
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

pub async fn list_streams(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<StreamResponse>>> {
    let user = state.require_user(&headers)?;
    let rows = if user.is_admin {
        sqlx::query("SELECT id, depot_id, name FROM streams ORDER BY name")
            .fetch_all(&state.db)
            .await?
    } else {
        sqlx::query(
            r#"
            SELECT DISTINCT s.id, s.depot_id, s.name
            FROM streams s
            JOIN depots d ON d.id = s.depot_id
            LEFT JOIN depot_user_permissions p
              ON p.depot_id = d.id AND p.user_id = $1
            WHERE d.owner_user_id = $1 OR p.role IN ('read', 'write', 'admin')
            ORDER BY s.name
            "#,
        )
        .bind(user.user_id)
        .fetch_all(&state.db)
        .await?
    };
    Ok(Json(
        rows.into_iter()
            .map(|row| StreamResponse {
                id: row.get("id"),
                depot_id: row.get("depot_id"),
                name: row.get("name"),
            })
            .collect(),
    ))
}

pub async fn resolve_stream_id(db: &sqlx::PgPool, depot_id: Uuid, stream: &str) -> AppResult<Uuid> {
    if let Ok(id) = Uuid::parse_str(stream) {
        return sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM streams WHERE id = $1 AND depot_id = $2",
        )
        .bind(id)
        .bind(depot_id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("stream not found: {stream}")));
    }
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM streams WHERE depot_id = $1 AND name = $2")
        .bind(depot_id)
        .bind(stream)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("stream not found: {stream}")))
}

fn map_unique_conflict(message: &'static str) -> impl FnOnce(sqlx::Error) -> AppError {
    move |error| match &error {
        sqlx::Error::Database(db_error) if db_error.is_unique_violation() => {
            AppError::conflict(message)
        }
        _ => AppError::Database(error),
    }
}
