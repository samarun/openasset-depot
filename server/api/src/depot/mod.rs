use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    error::{AppError, AppResult},
    idempotency,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDepotRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

pub async fn create_depot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateDepotRequest>,
) -> AppResult<Json<DepotResponse>> {
    let user = state.require_user(&headers)?;
    validate_name(&req.name)?;
    let response = idempotency::run(&state.db, &headers, &user, "depot_create", &req, || async {
        let mut tx = state.db.begin().await?;
        let row = sqlx::query(
            r#"
                INSERT INTO depots (name, description, owner_user_id)
                VALUES ($1, $2, $3)
                RETURNING id, name, description
                "#,
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(user.user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_unique_conflict("depot already exists"))?;

        let response = DepotResponse {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
        };
        sqlx::query(
            r#"
                INSERT INTO depot_user_permissions (depot_id, user_id, role, granted_by)
                VALUES ($1, $2, 'admin', $2)
                ON CONFLICT (depot_id, user_id) DO NOTHING
                "#,
        )
        .bind(response.id)
        .bind(user.user_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        audit::record(
            &state.db,
            audit::AuditEvent {
                actor_user_id: Some(user.user_id),
                ..audit::AuditEvent::new(
                    "depot_create",
                    serde_json::json!({ "name": response.name }),
                )
            },
        )
        .await?;
        Ok(response)
    })
    .await?;
    Ok(Json(response))
}

pub async fn list_depots(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<DepotResponse>>> {
    let user = state.require_user(&headers)?;
    let rows = if user.is_admin {
        sqlx::query("SELECT id, name, description FROM depots ORDER BY name")
            .fetch_all(&state.db)
            .await?
    } else {
        sqlx::query(
            r#"
            SELECT DISTINCT d.id, d.name, d.description
            FROM depots d
            LEFT JOIN effective_depot_permissions p
              ON p.depot_id = d.id AND p.user_id = $1
            WHERE d.owner_user_id = $1 OR p.role IN ('read', 'write', 'admin')
            ORDER BY d.name
            "#,
        )
        .bind(user.user_id)
        .fetch_all(&state.db)
        .await?
    };
    Ok(Json(
        rows.into_iter()
            .map(|row| DepotResponse {
                id: row.get("id"),
                name: row.get("name"),
                description: row.get("description"),
            })
            .collect(),
    ))
}

pub async fn resolve_depot_id(db: &sqlx::PgPool, depot: &str) -> AppResult<Uuid> {
    if let Ok(id) = Uuid::parse_str(depot) {
        return Ok(id);
    }
    let id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM depots WHERE name = $1")
        .bind(depot)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("depot not found: {depot}")))?;
    Ok(id)
}

pub fn validate_name(name: &str) -> AppResult<()> {
    if name.len() < 2 || name.len() > 96 {
        return Err(AppError::bad_request("name must be 2 to 96 characters"));
    }
    if name.trim() != name {
        return Err(AppError::bad_request(
            "name cannot start or end with a space",
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '))
    {
        return Err(AppError::bad_request(
            "name may contain letters, numbers, spaces, '.', '_' and '-'",
        ));
    }
    Ok(())
}

fn map_unique_conflict(message: &'static str) -> impl FnOnce(sqlx::Error) -> AppError {
    move |error| match &error {
        sqlx::Error::Database(db_error) if db_error.is_unique_violation() => {
            AppError::conflict(message)
        }
        _ => AppError::Database(error),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_name;

    #[test]
    fn studio_names_can_be_human_readable_without_accepting_path_syntax() {
        assert!(validate_name("Review QA Depot").is_ok());
        assert!(validate_name("main-v2").is_ok());
        assert!(validate_name(" trailing").is_err());
        assert!(validate_name("shows/feature").is_err());
    }
}
