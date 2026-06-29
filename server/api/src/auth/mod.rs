use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{extract::State, http::HeaderMap, Json};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
    pub is_admin: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    username: String,
    is_admin: bool,
    exp: usize,
}

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    ttl_seconds: i64,
}

impl JwtService {
    pub fn new(secret: String, ttl_seconds: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            ttl_seconds,
        }
    }

    pub fn encode(&self, user: &AuthUser) -> AppResult<String> {
        let expires_at = Utc::now() + Duration::seconds(self.ttl_seconds);
        let claims = Claims {
            sub: user.user_id,
            username: user.username.clone(),
            is_admin: user.is_admin,
            exp: expires_at.timestamp() as usize,
        };
        Ok(encode(&Header::default(), &claims, &self.encoding_key)?)
    }

    pub fn decode(&self, token: &str) -> AppResult<AuthUser> {
        let claims = decode::<Claims>(token, &self.decoding_key, &Validation::default())?.claims;
        Ok(AuthUser {
            user_id: claims.sub,
            username: claims.username,
            is_admin: claims.is_admin,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
    pub is_admin: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

pub async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateUserRequest>,
) -> AppResult<Json<UserResponse>> {
    validate_username(&req.username)?;
    if req.password.len() < 8 {
        return Err(AppError::bad_request(
            "password must be at least 8 characters",
        ));
    }

    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(0x4f41445f55534552_i64)
        .execute(&mut *tx)
        .await?;
    let existing_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&mut *tx)
        .await?;
    let requested_admin = req.is_admin.unwrap_or(false);
    let actor = if existing_users == 0 {
        None
    } else {
        let user = state.require_user(&headers)?;
        if !user.is_admin {
            return Err(AppError::Forbidden(
                "only admins can create users".to_string(),
            ));
        }
        Some(user)
    };

    let password_hash = hash_password(&req.password)?;
    let row = sqlx::query(
        r#"
        INSERT INTO users (username, display_name, password_hash, is_admin)
        VALUES ($1, $2, $3, $4)
        RETURNING id, username, display_name, is_admin
        "#,
    )
    .bind(req.username)
    .bind(req.display_name)
    .bind(password_hash)
    .bind(if existing_users == 0 {
        true
    } else {
        requested_admin
    })
    .fetch_one(&mut *tx)
    .await
    .map_err(map_unique_conflict("username already exists"))?;

    let response = UserResponse {
        id: row.get("id"),
        username: row.get("username"),
        display_name: row.get("display_name"),
        is_admin: row.get("is_admin"),
    };

    audit::record_tx(
        &mut tx,
        audit::AuditEvent {
            actor_user_id: actor.as_ref().map(|u| u.user_id),
            ..audit::AuditEvent::new(
                "user_create",
                serde_json::json!({ "username": response.username }),
            )
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(response))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let row = sqlx::query(
        "SELECT id, username, display_name, password_hash, is_admin FROM users WHERE username = $1",
    )
    .bind(&req.username)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        audit::record(
            &state.db,
            audit::AuditEvent::new(
                "failed_login",
                serde_json::json!({ "username": req.username }),
            ),
        )
        .await?;
        return Err(AppError::Unauthorized(
            "invalid username or password".to_string(),
        ));
    };

    let password_hash: String = row.get("password_hash");
    if !verify_password(&req.password, &password_hash)? {
        audit::record(
            &state.db,
            audit::AuditEvent::new(
                "failed_login",
                serde_json::json!({ "username": req.username }),
            ),
        )
        .await?;
        return Err(AppError::Unauthorized(
            "invalid username or password".to_string(),
        ));
    }

    let auth_user = AuthUser {
        user_id: row.get("id"),
        username: row.get("username"),
        is_admin: row.get("is_admin"),
    };
    let user = UserResponse {
        id: auth_user.user_id,
        username: auth_user.username.clone(),
        display_name: row.get("display_name"),
        is_admin: auth_user.is_admin,
    };
    let token = state.jwt.encode(&auth_user)?;

    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(auth_user.user_id),
            ..audit::AuditEvent::new(
                "login",
                serde_json::json!({ "username": auth_user.username }),
            )
        },
    )
    .await?;

    Ok(Json(LoginResponse { token, user }))
}

fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

fn verify_password(password: &str, password_hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(password_hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn validate_username(username: &str) -> AppResult<()> {
    if username.len() < 3 || username.len() > 64 {
        return Err(AppError::bad_request("username must be 3 to 64 characters"));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(AppError::bad_request(
            "username may contain letters, numbers, '.', '_' and '-'",
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
    use super::{hash_password, verify_password};

    #[test]
    fn password_hashes_verify() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash).unwrap());
        assert!(!verify_password("incorrect", &hash).unwrap());
    }
}
