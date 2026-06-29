use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

use crate::error::{AppError, AppResult};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub database_max_connections: u32,
    pub storage_root: PathBuf,
    pub jwt_secret: String,
    pub jwt_ttl_seconds: i64,
    pub max_upload_bytes: usize,
    pub request_timeout: Duration,
    pub chunk_size: usize,
    pub run_migrations: bool,
    pub cors_allowed_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> AppResult<Self> {
        let bind_addr = env::var("OAD_BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse()
            .map_err(|_| AppError::configuration("OAD_BIND_ADDR must be host:port"))?;
        let database_url = env::var("OAD_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| AppError::configuration("OAD_DATABASE_URL or DATABASE_URL is required"))?;
        let jwt_secret = env::var("OAD_JWT_SECRET")
            .map_err(|_| AppError::configuration("OAD_JWT_SECRET is required"))?;
        validate_jwt_secret(&jwt_secret)?;

        let database_max_connections = parse_env("OAD_DATABASE_MAX_CONNECTIONS", 10)?;
        let jwt_ttl_seconds = parse_env("OAD_JWT_TTL_SECONDS", 86_400)?;
        let max_upload_bytes = parse_env("OAD_MAX_UPLOAD_BYTES", 20 * 1024 * 1024 * 1024usize)?;
        let request_timeout_seconds = parse_env("OAD_REQUEST_TIMEOUT_SECONDS", 120)?;
        let chunk_size = parse_env("OAD_CHUNK_SIZE_BYTES", 4 * 1024 * 1024usize)?;
        if database_max_connections == 0 {
            return Err(AppError::configuration(
                "OAD_DATABASE_MAX_CONNECTIONS must be greater than zero",
            ));
        }
        if !(60..=604_800).contains(&jwt_ttl_seconds) {
            return Err(AppError::configuration(
                "OAD_JWT_TTL_SECONDS must be between 60 and 604800",
            ));
        }
        if max_upload_bytes == 0 {
            return Err(AppError::configuration(
                "OAD_MAX_UPLOAD_BYTES must be greater than zero",
            ));
        }
        if request_timeout_seconds == 0 {
            return Err(AppError::configuration(
                "OAD_REQUEST_TIMEOUT_SECONDS must be greater than zero",
            ));
        }
        if !(64 * 1024..=64 * 1024 * 1024).contains(&chunk_size) {
            return Err(AppError::configuration(
                "OAD_CHUNK_SIZE_BYTES must be between 65536 and 67108864",
            ));
        }

        Ok(Self {
            bind_addr,
            database_url,
            database_max_connections,
            storage_root: env::var("OAD_STORAGE_ROOT")
                .unwrap_or_else(|_| "./data/objects".to_string())
                .into(),
            jwt_secret,
            jwt_ttl_seconds,
            max_upload_bytes,
            request_timeout: Duration::from_secs(request_timeout_seconds),
            chunk_size,
            run_migrations: env::var("OAD_RUN_MIGRATIONS")
                .map(|value| value != "false" && value != "0")
                .unwrap_or(true),
            cors_allowed_origins: parse_cors_origins(
                &env::var("OAD_CORS_ALLOWED_ORIGINS").unwrap_or_else(|_| {
                    "http://127.0.0.1:5173,http://localhost:5173,tauri://localhost,http://tauri.localhost"
                        .to_string()
                }),
            )?,
        })
    }
}

fn parse_cors_origins(value: &str) -> AppResult<Vec<String>> {
    let origins = value
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            if origin == "*"
                || (!origin.starts_with("http://")
                    && !origin.starts_with("https://")
                    && !origin.starts_with("tauri://"))
            {
                return Err(AppError::configuration(
                    "OAD_CORS_ALLOWED_ORIGINS must contain explicit HTTP(S) or Tauri origins",
                ));
            }
            origin
                .parse::<axum::http::HeaderValue>()
                .map_err(|_| AppError::configuration("OAD_CORS_ALLOWED_ORIGINS is invalid"))?;
            Ok(origin.to_string())
        })
        .collect::<AppResult<Vec<_>>>()?;
    if origins.is_empty() {
        return Err(AppError::configuration(
            "OAD_CORS_ALLOWED_ORIGINS requires at least one explicit origin",
        ));
    }
    Ok(origins)
}

fn validate_jwt_secret(secret: &str) -> AppResult<()> {
    if secret.len() < 32 {
        return Err(AppError::configuration(
            "OAD_JWT_SECRET must be at least 32 characters",
        ));
    }
    let lower = secret.to_ascii_lowercase();
    if lower.contains("replace-with") || lower.contains("change-me") {
        return Err(AppError::configuration(
            "OAD_JWT_SECRET must be changed from the example placeholder",
        ));
    }
    Ok(())
}

fn parse_env<T>(key: &str, default: T) -> AppResult<T>
where
    T: std::str::FromStr,
{
    match env::var(key) {
        Ok(value) => value
            .parse()
            .map_err(|_| AppError::configuration(format!("{key} has an invalid value"))),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_cors_origins, validate_jwt_secret};

    #[test]
    fn jwt_secret_rejects_short_values() {
        assert!(validate_jwt_secret("too-short").is_err());
    }

    #[test]
    fn jwt_secret_rejects_example_placeholders() {
        assert!(validate_jwt_secret("replace-with-at-least-32-random-characters").is_err());
        assert!(validate_jwt_secret("change-me-to-a-real-random-jwt-secret").is_err());
    }

    #[test]
    fn jwt_secret_accepts_long_non_placeholder_values() {
        assert!(validate_jwt_secret("0123456789abcdef0123456789abcdef").is_ok());
    }

    #[test]
    fn cors_rejects_wildcards_and_non_origins() {
        assert!(parse_cors_origins("*").is_err());
        assert!(parse_cors_origins("localhost:5173").is_err());
        assert!(parse_cors_origins("https://depot.example.test,tauri://localhost").is_ok());
    }
}
