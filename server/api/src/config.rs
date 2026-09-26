use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

use crate::error::{AppError, AppResult};

/// Where blob chunks are stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageBackend {
    Local,
    /// Any S3-compatible bucket. `endpoint_url` is required for MinIO and other
    /// non-AWS implementations and forces path-style addressing.
    S3 {
        bucket: String,
        prefix: String,
        endpoint_url: Option<String>,
    },
}

/// Single sign-on through an OpenID Connect provider.
///
/// Optional: when absent the server stays on username/password JWT auth, so a
/// small studio is never forced to run an identity provider.
#[derive(Clone, Debug)]
pub struct OidcConfig {
    /// Issuer URL. Discovery appends `/.well-known/openid-configuration`.
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    /// Extra scopes beyond `openid profile email`, space separated.
    pub extra_scopes: Vec<String>,
    /// ID-token claim listing the user's directory groups. Each value is matched
    /// against `groups.external_id`, so depot access follows the directory.
    pub groups_claim: Option<String>,
    /// Claim naming users who should receive system-admin rights.
    pub admin_group: Option<String>,
}

impl OidcConfig {
    pub fn scope(&self) -> String {
        let mut scopes = vec!["openid".to_string(), "profile".into(), "email".into()];
        for scope in &self.extra_scopes {
            if !scopes.contains(scope) {
                scopes.push(scope.clone());
            }
        }
        scopes.join(" ")
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub database_max_connections: u32,
    pub storage_root: PathBuf,
    pub storage_backend: StorageBackend,
    /// How often the background worker verifies storage and reclaims orphans.
    /// Zero disables the worker.
    pub integrity_interval: Duration,
    /// Blobs sampled per integrity sweep. Keeps a sweep bounded on large depots.
    pub integrity_sample_size: usize,
    pub jwt_secret: String,
    pub jwt_ttl_seconds: i64,
    pub max_upload_bytes: usize,
    pub request_timeout: Duration,
    pub chunk_size: usize,
    pub run_migrations: bool,
    pub allow_signups: bool,
    pub cors_allowed_origins: Vec<String>,
    /// `None` when no identity provider is configured.
    pub oidc: Option<OidcConfig>,
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

        let integrity_interval_seconds: u64 = parse_env("OAD_INTEGRITY_INTERVAL_SECONDS", 0)?;
        if integrity_interval_seconds != 0 && integrity_interval_seconds < 60 {
            return Err(AppError::configuration(
                "OAD_INTEGRITY_INTERVAL_SECONDS must be 0 (disabled) or at least 60",
            ));
        }
        let integrity_sample_size = parse_env("OAD_INTEGRITY_SAMPLE_SIZE", 250usize)?;
        if integrity_sample_size == 0 {
            return Err(AppError::configuration(
                "OAD_INTEGRITY_SAMPLE_SIZE must be greater than zero",
            ));
        }

        Ok(Self {
            bind_addr,
            database_url,
            database_max_connections,
            storage_root: env::var("OAD_STORAGE_ROOT")
                .unwrap_or_else(|_| "./data/objects".to_string())
                .into(),
            storage_backend: parse_storage_backend()?,
            integrity_interval: Duration::from_secs(integrity_interval_seconds),
            integrity_sample_size,
            jwt_secret,
            jwt_ttl_seconds,
            max_upload_bytes,
            request_timeout: Duration::from_secs(request_timeout_seconds),
            chunk_size,
            run_migrations: env::var("OAD_RUN_MIGRATIONS")
                .map(|value| value != "false" && value != "0")
                .unwrap_or(true),
            allow_signups: parse_bool_env("OAD_ALLOW_SIGNUPS", false)?,
            cors_allowed_origins: parse_cors_origins(
                &env::var("OAD_CORS_ALLOWED_ORIGINS").unwrap_or_else(|_| {
                    "http://127.0.0.1:5173,http://localhost:5173,tauri://localhost,http://tauri.localhost"
                        .to_string()
                }),
            )?,
            oidc: parse_oidc()?,
        })
    }
}

/// Reads the optional OIDC block.
///
/// All four core values are required together: a half-configured provider would
/// otherwise present a Sign in with SSO button that cannot complete.
fn parse_oidc() -> AppResult<Option<OidcConfig>> {
    let issuer = match env::var("OAD_OIDC_ISSUER") {
        Ok(value) if !value.trim().is_empty() => value.trim().trim_end_matches('/').to_string(),
        _ => return Ok(None),
    };
    if !issuer.starts_with("https://") && !issuer.starts_with("http://") {
        return Err(AppError::configuration(
            "OAD_OIDC_ISSUER must be an HTTP(S) URL",
        ));
    }
    let required = |key: &str| {
        env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::configuration(format!("{key} is required when OAD_OIDC_ISSUER is set"))
            })
    };
    let redirect_uri = required("OAD_OIDC_REDIRECT_URI")?;
    if !redirect_uri.contains("://") {
        return Err(AppError::configuration(
            "OAD_OIDC_REDIRECT_URI must be an absolute URI",
        ));
    }

    Ok(Some(OidcConfig {
        issuer,
        client_id: required("OAD_OIDC_CLIENT_ID")?,
        client_secret: required("OAD_OIDC_CLIENT_SECRET")?,
        redirect_uri,
        extra_scopes: env::var("OAD_OIDC_EXTRA_SCOPES")
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect(),
        groups_claim: env::var("OAD_OIDC_GROUPS_CLAIM")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        admin_group: env::var("OAD_OIDC_ADMIN_GROUP")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    }))
}

fn parse_storage_backend() -> AppResult<StorageBackend> {
    let kind = env::var("OAD_STORAGE_BACKEND").unwrap_or_else(|_| "local".to_string());
    match kind.trim().to_ascii_lowercase().as_str() {
        "local" => Ok(StorageBackend::Local),
        "s3" => {
            let bucket = env::var("OAD_S3_BUCKET").map_err(|_| {
                AppError::configuration("OAD_S3_BUCKET is required when OAD_STORAGE_BACKEND=s3")
            })?;
            if bucket.trim().is_empty() {
                return Err(AppError::configuration("OAD_S3_BUCKET must not be empty"));
            }
            Ok(StorageBackend::S3 {
                bucket,
                prefix: env::var("OAD_S3_PREFIX").unwrap_or_default(),
                endpoint_url: env::var("OAD_S3_ENDPOINT_URL")
                    .ok()
                    .filter(|v| !v.is_empty()),
            })
        }
        _ => Err(AppError::configuration(
            "OAD_STORAGE_BACKEND must be local or s3",
        )),
    }
}

fn parse_bool_env(key: &str, default: bool) -> AppResult<bool> {
    match env::var(key) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(AppError::configuration(format!(
                "{key} must be true or false"
            ))),
        },
        Err(_) => Ok(default),
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
    use super::{parse_bool_env, parse_cors_origins, validate_jwt_secret};

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

    #[test]
    fn boolean_environment_parser_rejects_ambiguous_values() {
        std::env::set_var("OAD_TEST_BOOLEAN", "sometimes");
        assert!(parse_bool_env("OAD_TEST_BOOLEAN", false).is_err());
        std::env::remove_var("OAD_TEST_BOOLEAN");
    }
}
