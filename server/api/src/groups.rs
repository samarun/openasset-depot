//! Group administration and group-based depot grants.
//!
//! Groups let a studio manage access by team ("lighting", "layout") instead of
//! per-artist rows that drift as people move between shows. A user's effective
//! depot role is the strongest of their direct grant and any grant held by a
//! group they belong to; that resolution lives in the
//! `effective_depot_permissions` view so every authorization path agrees.

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
    error::{AppError, AppResult},
    idempotency,
    permissions::{ensure_depot_permission, DepotPermission},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub description: Option<String>,
    /// Stable identifier from an external directory, when one manages this group.
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub external_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupMemberRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberResponse {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GrantGroupPermissionRequest {
    pub group_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPermissionResponse {
    pub depot_id: Uuid,
    pub group_id: Uuid,
    pub group_name: String,
    pub role: String,
    pub granted_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateGroupRequest>,
) -> AppResult<Json<GroupResponse>> {
    let user = state.require_user(&headers)?;
    require_system_admin(&user)?;
    let name = req.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::bad_request(
            "group name must be 1 to 100 characters",
        ));
    }

    let response = idempotency::run(&state.db, &headers, &user, "group_create", &req, || async {
        let row = sqlx::query(
            r#"
        INSERT INTO groups (name, description, external_id)
        VALUES ($1, $2, $3)
        RETURNING id, name, description, external_id, created_at
        "#,
        )
        .bind(name)
        .bind(req.description.as_deref())
        .bind(req.external_id.as_deref())
        .fetch_one(&state.db)
        .await
        .map_err(|error| match &error {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                AppError::conflict("a group with that name or external id already exists")
            }
            _ => AppError::Database(error),
        })?;

        let response = group_response(row);
        audit::record(
            &state.db,
            audit::AuditEvent {
                actor_user_id: Some(user.user_id),
                ..audit::AuditEvent::new(
                    "group_create",
                    serde_json::json!({ "group_id": response.id, "name": response.name }),
                )
            },
        )
        .await?;
        Ok(response)
    })
    .await?;
    Ok(Json(response))
}

pub async fn list_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<GroupResponse>>> {
    let user = state.require_user(&headers)?;
    require_system_admin(&user)?;
    let rows = sqlx::query(
        "SELECT id, name, description, external_id, created_at FROM groups ORDER BY name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(group_response).collect()))
}

pub async fn list_group_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> AppResult<Json<Vec<GroupMemberResponse>>> {
    let user = state.require_user(&headers)?;
    require_system_admin(&user)?;
    let rows = sqlx::query(
        r#"
        SELECT ug.group_id, ug.user_id, u.username, ug.created_at
        FROM user_groups ug
        JOIN users u ON u.id = ug.user_id
        WHERE ug.group_id = $1
        ORDER BY u.username
        "#,
    )
    .bind(group_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| GroupMemberResponse {
                group_id: row.get("group_id"),
                user_id: row.get("user_id"),
                username: row.get("username"),
                created_at: row.get("created_at"),
            })
            .collect(),
    ))
}

pub async fn add_group_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
    Json(req): Json<GroupMemberRequest>,
) -> AppResult<Json<GroupMemberResponse>> {
    let user = state.require_user(&headers)?;
    require_system_admin(&user)?;

    let request = serde_json::json!({ "group_id": group_id, "user_id": req.user_id });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "group_member_add",
        &request,
        || async {
            // Re-adding an existing member is a no-op rather than an error, so a
            // directory sync can be replayed safely.
            let row = sqlx::query(
                r#"
        WITH inserted AS (
            INSERT INTO user_groups (user_id, group_id)
            VALUES ($1, $2)
            ON CONFLICT (user_id, group_id) DO NOTHING
            RETURNING user_id, group_id, created_at
        )
        SELECT ug.group_id, ug.user_id, u.username, ug.created_at
        FROM (
            SELECT * FROM inserted
            UNION ALL
            SELECT user_id, group_id, created_at FROM user_groups
            WHERE user_id = $1 AND group_id = $2
        ) ug
        JOIN users u ON u.id = ug.user_id
        LIMIT 1
        "#,
            )
            .bind(req.user_id)
            .bind(group_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|error| match &error {
                sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
                    AppError::NotFound("group or user not found".to_string())
                }
                _ => AppError::Database(error),
            })?
            .ok_or_else(|| AppError::NotFound("group or user not found".to_string()))?;

            let response = GroupMemberResponse {
                group_id: row.get("group_id"),
                user_id: row.get("user_id"),
                username: row.get("username"),
                created_at: row.get("created_at"),
            };
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    ..audit::AuditEvent::new(
                        "group_member_add",
                        serde_json::json!({ "group_id": group_id, "user_id": response.user_id }),
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

pub async fn remove_group_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((group_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let actor = state.require_user(&headers)?;
    require_system_admin(&actor)?;
    // With a key, a retried delete replays the original success instead of the
    // 404 it would otherwise get once the row is already gone.
    let request = serde_json::json!({ "group_id": group_id, "user_id": user_id });
    let response = idempotency::run(
        &state.db,
        &headers,
        &actor,
        "group_member_remove",
        &request,
        || async {
            let removed =
                sqlx::query("DELETE FROM user_groups WHERE group_id = $1 AND user_id = $2")
                    .bind(group_id)
                    .bind(user_id)
                    .execute(&state.db)
                    .await?
                    .rows_affected();
            if removed == 0 {
                return Err(AppError::NotFound("group membership not found".to_string()));
            }
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(actor.user_id),
                    ..audit::AuditEvent::new(
                        "group_member_remove",
                        serde_json::json!({ "group_id": group_id, "user_id": user_id }),
                    )
                },
            )
            .await?;
            Ok(serde_json::json!({ "removed": true }))
        },
    )
    .await?;
    Ok(Json(response))
}

pub async fn grant_group_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(depot_id): Path<Uuid>,
    Json(req): Json<GrantGroupPermissionRequest>,
) -> AppResult<Json<GroupPermissionResponse>> {
    let user = state.require_user(&headers)?;
    ensure_depot_permission(&state.db, &user, depot_id, DepotPermission::Admin).await?;
    let role = DepotPermission::parse_role(&req.role)?;

    let request = serde_json::json!({
        "depot_id": depot_id,
        "group_id": req.group_id,
        "role": role,
    });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "depot_group_permission_grant",
        &request,
        || async {
            let row = sqlx::query(
                r#"
        WITH upserted AS (
            INSERT INTO depot_group_permissions (depot_id, group_id, role, granted_by)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (depot_id, group_id)
            DO UPDATE SET role = EXCLUDED.role, granted_by = EXCLUDED.granted_by, updated_at = now()
            RETURNING depot_id, group_id, role, granted_by, created_at, updated_at
        )
        SELECT u.depot_id, u.group_id, g.name AS group_name, u.role,
               u.granted_by, u.created_at, u.updated_at
        FROM upserted u
        JOIN groups g ON g.id = u.group_id
        "#,
            )
            .bind(depot_id)
            .bind(req.group_id)
            .bind(role)
            .bind(user.user_id)
            .fetch_one(&state.db)
            .await
            .map_err(|error| match &error {
                sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
                    AppError::NotFound("depot or group not found".to_string())
                }
                _ => AppError::Database(error),
            })?;

            let response = group_permission_response(row);
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    ..audit::AuditEvent::new(
                        "depot_group_permission_grant",
                        serde_json::json!({
                            "depot_id": depot_id,
                            "group_id": response.group_id,
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

pub async fn list_group_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(depot_id): Path<Uuid>,
) -> AppResult<Json<Vec<GroupPermissionResponse>>> {
    let user = state.require_user(&headers)?;
    ensure_depot_permission(&state.db, &user, depot_id, DepotPermission::Admin).await?;
    let rows = sqlx::query(
        r#"
        SELECT p.depot_id, p.group_id, g.name AS group_name, p.role,
               p.granted_by, p.created_at, p.updated_at
        FROM depot_group_permissions p
        JOIN groups g ON g.id = p.group_id
        WHERE p.depot_id = $1
        ORDER BY g.name
        "#,
    )
    .bind(depot_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter().map(group_permission_response).collect(),
    ))
}

pub async fn revoke_group_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((depot_id, group_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let user = state.require_user(&headers)?;
    ensure_depot_permission(&state.db, &user, depot_id, DepotPermission::Admin).await?;
    let request = serde_json::json!({ "depot_id": depot_id, "group_id": group_id });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "depot_group_permission_revoke",
        &request,
        || async {
            let removed = sqlx::query(
                "DELETE FROM depot_group_permissions WHERE depot_id = $1 AND group_id = $2",
            )
            .bind(depot_id)
            .bind(group_id)
            .execute(&state.db)
            .await?
            .rows_affected();
            if removed == 0 {
                return Err(AppError::NotFound("group grant not found".to_string()));
            }
            audit::record(
                &state.db,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    ..audit::AuditEvent::new(
                        "depot_group_permission_revoke",
                        serde_json::json!({ "depot_id": depot_id, "group_id": group_id }),
                    )
                },
            )
            .await?;
            Ok(serde_json::json!({ "revoked": true }))
        },
    )
    .await?;
    Ok(Json(response))
}

/// Group administration is studio-wide, so it is reserved for system admins
/// rather than per-depot admins who can only see part of the picture.
fn require_system_admin(user: &crate::auth::AuthUser) -> AppResult<()> {
    if user.is_admin {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "only system admins can manage groups".to_string(),
        ))
    }
}

fn group_response(row: sqlx::postgres::PgRow) -> GroupResponse {
    GroupResponse {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        external_id: row.get("external_id"),
        created_at: row.get("created_at"),
    }
}

fn group_permission_response(row: sqlx::postgres::PgRow) -> GroupPermissionResponse {
    GroupPermissionResponse {
        depot_id: row.get("depot_id"),
        group_id: row.get("group_id"),
        group_name: row.get("group_name"),
        role: row.get("role"),
        granted_by: row.get("granted_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
