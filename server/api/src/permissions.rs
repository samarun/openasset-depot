use axum::{
    extract::{Path, State},
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
    auth::AuthUser,
    error::{AppError, AppResult},
    idempotency,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepotPermission {
    Read,
    Write,
    Admin,
}

impl DepotPermission {
    fn from_role(role: &str) -> AppResult<Self> {
        match role {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            "admin" => Ok(Self::Admin),
            _ => Err(AppError::bad_request("role must be read, write, or admin")),
        }
    }

    fn as_role(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Admin => "admin",
        }
    }

    /// Validates a role string from a request and returns its canonical form.
    pub fn parse_role(role: &str) -> AppResult<&'static str> {
        Ok(Self::from_role(role)?.as_role())
    }

    fn rank(self) -> u8 {
        match self {
            Self::Read => 1,
            Self::Write => 2,
            Self::Admin => 3,
        }
    }

    fn allows(self, required: Self) -> bool {
        self.rank() >= required.rank()
    }
}

#[derive(Debug, Deserialize)]
pub struct GrantDepotPermissionRequest {
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotPermissionResponse {
    pub depot_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub granted_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn ensure_depot_permission(
    db: &sqlx::PgPool,
    user: &AuthUser,
    depot_id: Uuid,
    required: DepotPermission,
) -> AppResult<()> {
    if user.is_admin {
        return Ok(());
    }

    let owner_user_id: Option<Uuid> =
        sqlx::query_scalar("SELECT owner_user_id FROM depots WHERE id = $1")
            .bind(depot_id)
            .fetch_optional(db)
            .await?
            .ok_or_else(|| AppError::NotFound("depot not found".to_string()))?;
    if owner_user_id == Some(user.user_id) {
        return Ok(());
    }

    // Reads the combined view so a grant held through a group counts exactly
    // as much as one made directly to the user.
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM effective_depot_permissions WHERE depot_id = $1 AND user_id = $2",
    )
    .bind(depot_id)
    .bind(user.user_id)
    .fetch_optional(db)
    .await?;
    let Some(role) = role else {
        return Err(AppError::Forbidden(
            "user does not have access to this depot".to_string(),
        ));
    };
    let granted = DepotPermission::from_role(&role)?;
    if granted.allows(required) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "{} permission is required for this depot",
            required.as_role()
        )))
    }
}

pub async fn grant_depot_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(depot_id): Path<Uuid>,
    Json(req): Json<GrantDepotPermissionRequest>,
) -> AppResult<Json<DepotPermissionResponse>> {
    let user = state.require_user(&headers)?;
    ensure_depot_permission(&state.db, &user, depot_id, DepotPermission::Admin).await?;
    let permission = DepotPermission::from_role(&req.role)?;
    // The depot comes from the path, so it has to join the hashed request or the
    // same key would replay across depots.
    let request = serde_json::json!({
        "depot_id": depot_id,
        "user_id": req.user_id,
        "role": permission.as_role(),
    });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "depot_permission_grant",
        &request,
        || async {
            let row = sqlx::query(
                r#"
        INSERT INTO depot_user_permissions (depot_id, user_id, role, granted_by)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (depot_id, user_id)
        DO UPDATE SET role = EXCLUDED.role, granted_by = EXCLUDED.granted_by, updated_at = now()
        RETURNING depot_id, user_id, role, granted_by, created_at, updated_at
        "#,
            )
            .bind(depot_id)
            .bind(req.user_id)
            .bind(permission.as_role())
            .bind(user.user_id)
            .fetch_one(&state.db)
            .await?;
            let response = permission_response(row);
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    ..audit::AuditEvent::new(
                        "depot_permission_grant",
                        serde_json::json!({
                            "depot_id": depot_id,
                            "user_id": response.user_id,
                            "role": response.role,
                        }),
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

pub async fn list_depot_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(depot_id): Path<Uuid>,
) -> AppResult<Json<Vec<DepotPermissionResponse>>> {
    let user = state.require_user(&headers)?;
    ensure_depot_permission(&state.db, &user, depot_id, DepotPermission::Admin).await?;
    let rows = sqlx::query(
        r#"
        SELECT depot_id, user_id, role, granted_by, created_at, updated_at
        FROM depot_user_permissions
        WHERE depot_id = $1
        ORDER BY role DESC, created_at ASC
        "#,
    )
    .bind(depot_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(permission_response).collect()))
}

fn permission_response(row: sqlx::postgres::PgRow) -> DepotPermissionResponse {
    DepotPermissionResponse {
        depot_id: row.get("depot_id"),
        user_id: row.get("user_id"),
        role: row.get("role"),
        granted_by: row.get("granted_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
