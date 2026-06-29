use std::future::Future;

use axum::http::{HeaderMap, HeaderValue};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
};

const IDEMPOTENCY_KEY: &str = "idempotency-key";

pub async fn run<B, T, F, Fut>(
    db: &sqlx::PgPool,
    headers: &HeaderMap,
    user: &AuthUser,
    operation: &str,
    request: &B,
    action: F,
) -> AppResult<T>
where
    B: Serialize + ?Sized,
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: Future<Output = AppResult<T>>,
{
    match prepare(db, headers, user, operation, request).await? {
        IdempotencyDecision::Replay(response) => Ok(response),
        IdempotencyDecision::Execute(reservation) => {
            let result = action().await;
            match (&reservation, &result) {
                (Some(reservation), Ok(response)) => {
                    reservation.store_response(db, response).await?;
                }
                (Some(reservation), Err(_)) => {
                    let _ = reservation.clear(db).await;
                }
                _ => {}
            }
            result
        }
    }
}

enum IdempotencyDecision<T> {
    Replay(T),
    Execute(Option<IdempotencyReservation>),
}

struct IdempotencyReservation {
    key: String,
    actor_user_id: Uuid,
    request_hash: String,
}

async fn prepare<B, T>(
    db: &sqlx::PgPool,
    headers: &HeaderMap,
    user: &AuthUser,
    operation: &str,
    request: &B,
) -> AppResult<IdempotencyDecision<T>>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    let Some(key) = headers.get(IDEMPOTENCY_KEY) else {
        return Ok(IdempotencyDecision::Execute(None));
    };
    let key = parse_key(key)?;
    let request_hash = request_hash(operation, request)?;

    let inserted = sqlx::query(
        r#"
        INSERT INTO idempotency_keys (key, actor_user_id, request_hash)
        VALUES ($1, $2, $3)
        ON CONFLICT (key) DO NOTHING
        RETURNING key
        "#,
    )
    .bind(&key)
    .bind(user.user_id)
    .bind(&request_hash)
    .fetch_optional(db)
    .await?;

    if inserted.is_some() {
        return Ok(IdempotencyDecision::Execute(Some(IdempotencyReservation {
            key,
            actor_user_id: user.user_id,
            request_hash,
        })));
    }

    let row = sqlx::query(
        "SELECT actor_user_id, request_hash, response FROM idempotency_keys WHERE key = $1",
    )
    .bind(&key)
    .fetch_one(db)
    .await?;
    let actor_user_id: Option<Uuid> = row.get("actor_user_id");
    if actor_user_id != Some(user.user_id) {
        return Err(AppError::Forbidden(
            "idempotency key belongs to a different user".to_string(),
        ));
    }
    let stored_hash: String = row.get("request_hash");
    if stored_hash != request_hash {
        return Err(AppError::conflict(
            "idempotency key was already used with a different request",
        ));
    }
    let response: Option<Value> = row.get("response");
    let Some(response) = response else {
        return Err(AppError::conflict("idempotency key is already processing"));
    };
    Ok(IdempotencyDecision::Replay(serde_json::from_value(
        response,
    )?))
}

impl IdempotencyReservation {
    async fn store_response<T: Serialize>(&self, db: &sqlx::PgPool, response: &T) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE idempotency_keys
            SET response = $1
            WHERE key = $2 AND actor_user_id = $3 AND request_hash = $4
            "#,
        )
        .bind(serde_json::to_value(response)?)
        .bind(&self.key)
        .bind(self.actor_user_id)
        .bind(&self.request_hash)
        .execute(db)
        .await?;
        Ok(())
    }

    async fn clear(&self, db: &sqlx::PgPool) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM idempotency_keys WHERE key = $1 AND actor_user_id = $2 AND request_hash = $3 AND response IS NULL",
        )
        .bind(&self.key)
        .bind(self.actor_user_id)
        .bind(&self.request_hash)
        .execute(db)
        .await?;
        Ok(())
    }
}

fn parse_key(value: &HeaderValue) -> AppResult<String> {
    let key = value
        .to_str()
        .map_err(|_| AppError::bad_request("Idempotency-Key must be valid ASCII"))?
        .trim();
    if key.is_empty() || key.len() > 200 {
        return Err(AppError::bad_request(
            "Idempotency-Key must be 1 to 200 characters",
        ));
    }
    Ok(key.to_string())
}

fn request_hash<B: Serialize + ?Sized>(operation: &str, request: &B) -> AppResult<String> {
    let body = serde_json::to_vec(request)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(operation.as_bytes());
    hasher.update(&body);
    Ok(hasher.finalize().to_hex().to_string())
}
