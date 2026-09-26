//! Single sign-on through an OpenID Connect provider.
//!
//! SSO users become ordinary rows in `users`, so every existing permission,
//! lock, and audit query keeps working unchanged. Only authentication differs:
//! a verified `(issuer, subject)` pair replaces a password hash, and the
//! resulting session is the same OAD JWT that password login issues.
//!
//! The flow is authorization code with PKCE. The desktop app opens the
//! authorization URL in a browser, receives the redirect, and posts the code
//! back here; the server holds the client secret and never exposes it.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{extract::State, Json};
use base64::Engine;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    config::OidcConfig,
    error::{AppError, AppResult},
};

use super::{AuthUser, LoginResponse, UserResponse};

/// How long an authorization request may sit before its state is rejected.
///
/// Short enough to bound replay, long enough for a real person to type a
/// password and complete a second factor.
const STATE_TTL: Duration = Duration::from_secs(10 * 60);

/// Cache lifetime for the discovery document and signing keys.
const METADATA_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Serialize)]
pub struct OidcStatusResponse {
    pub enabled: bool,
    /// Label for the sign-in button, so the UI does not hard-code a vendor name.
    pub provider: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OidcStartResponse {
    pub authorization_url: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackRequest {
    pub code: String,
    pub state: String,
}

/// Provider endpoints from the discovery document.
#[derive(Debug, Clone, Deserialize)]
struct ProviderMetadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Debug, Clone, Deserialize)]
struct Jwk {
    kid: Option<String>,
    #[serde(rename = "kty")]
    key_type: String,
    #[serde(rename = "alg")]
    algorithm: Option<String>,
    // RSA
    n: Option<String>,
    e: Option<String>,
    // EC
    crv: Option<String>,
    x: Option<String>,
    y: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: String,
}

/// One in-flight authorization request.
///
/// The PKCE verifier and nonce stay server-side: the client only ever holds the
/// opaque state value, so a stolen redirect URL cannot complete a login.
struct PendingAuth {
    code_verifier: String,
    nonce: String,
    created_at: Instant,
}

/// Server-held OIDC state: pending authorizations plus cached provider metadata.
#[derive(Default)]
pub struct OidcStore {
    pending: Mutex<HashMap<String, PendingAuth>>,
    metadata: Mutex<Option<(ProviderMetadata, Vec<Jwk>, Instant)>>,
}

impl std::fmt::Debug for OidcStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OidcStore")
    }
}

/// Reports whether SSO is available, so the desktop only offers a working button.
pub async fn oidc_status(State(state): State<AppState>) -> Json<OidcStatusResponse> {
    Json(match state.config.oidc.as_ref() {
        Some(oidc) => OidcStatusResponse {
            enabled: true,
            provider: Some(provider_label(&oidc.issuer)),
        },
        None => OidcStatusResponse {
            enabled: false,
            provider: None,
        },
    })
}

/// Begins a login and returns the URL the client should open in a browser.
pub async fn oidc_start(State(state): State<AppState>) -> AppResult<Json<OidcStartResponse>> {
    let oidc = require_oidc(&state)?;
    let metadata = provider_metadata(&state, oidc).await?.0;

    let state_token = random_token();
    let code_verifier = random_token();
    let nonce = random_token();
    let challenge = code_challenge(&code_verifier);

    {
        let mut pending = state
            .oidc
            .pending
            .lock()
            .map_err(|_| AppError::internal("oidc state is poisoned"))?;
        prune_pending(&mut pending);
        pending.insert(
            state_token.clone(),
            PendingAuth {
                code_verifier,
                nonce: nonce.clone(),
                created_at: Instant::now(),
            },
        );
    }

    let authorization_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
        metadata.authorization_endpoint,
        urlencode(&oidc.client_id),
        urlencode(&oidc.redirect_uri),
        urlencode(&oidc.scope()),
        urlencode(&state_token),
        urlencode(&nonce),
        urlencode(&challenge),
    );
    Ok(Json(OidcStartResponse {
        authorization_url,
        state: state_token,
    }))
}

/// Exchanges an authorization code for an OAD session.
pub async fn oidc_callback(
    State(state): State<AppState>,
    Json(req): Json<OidcCallbackRequest>,
) -> AppResult<Json<LoginResponse>> {
    let oidc = require_oidc(&state)?;
    // Consumed on use: an authorization code and its state are single-use.
    let pending = {
        let mut pending = state
            .oidc
            .pending
            .lock()
            .map_err(|_| AppError::internal("oidc state is poisoned"))?;
        prune_pending(&mut pending);
        pending
            .remove(&req.state)
            .ok_or_else(|| AppError::Unauthorized("sign-in request expired".to_string()))?
    };

    let (metadata, keys) = provider_metadata(&state, oidc).await?;
    let client = http_client()?;
    let token: TokenResponse = client
        .post(&metadata.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", req.code.as_str()),
            ("redirect_uri", oidc.redirect_uri.as_str()),
            ("client_id", oidc.client_id.as_str()),
            ("client_secret", oidc.client_secret.as_str()),
            ("code_verifier", pending.code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|error| AppError::Unauthorized(format!("token exchange failed: {error}")))?
        .error_for_status()
        .map_err(|error| AppError::Unauthorized(format!("token exchange rejected: {error}")))?
        .json()
        .await
        .map_err(|error| AppError::Unauthorized(format!("token response invalid: {error}")))?;

    let claims = verify_id_token(&token.id_token, oidc, &metadata, &keys, &pending.nonce)?;
    let session = provision_user(&state, oidc, &claims).await?;
    Ok(Json(session))
}

/// Verified identity claims from an ID token.
struct IdentityClaims {
    subject: String,
    username: String,
    display_name: Option<String>,
    groups: Vec<String>,
}

/// Validates an ID token's signature, issuer, audience, expiry, and nonce.
///
/// Every one of these matters: skipping the nonce would allow a token minted for
/// a different login to be replayed here, and skipping the audience would accept
/// a token issued for an unrelated client at the same provider.
fn verify_id_token(
    id_token: &str,
    oidc: &OidcConfig,
    metadata: &ProviderMetadata,
    keys: &[Jwk],
    expected_nonce: &str,
) -> AppResult<IdentityClaims> {
    let header = decode_header(id_token)
        .map_err(|error| AppError::Unauthorized(format!("id token header invalid: {error}")))?;
    let key = keys
        .iter()
        .find(|candidate| match (&candidate.kid, &header.kid) {
            (Some(candidate_kid), Some(token_kid)) => candidate_kid == token_kid,
            // A provider publishing a single unnamed key is still usable.
            _ => keys.len() == 1,
        })
        .ok_or_else(|| {
            AppError::Unauthorized("id token was signed by an unknown key".to_string())
        })?;

    let decoding_key = decoding_key(key)?;
    let algorithm = key
        .algorithm
        .as_deref()
        .and_then(|alg| alg.parse::<Algorithm>().ok())
        .unwrap_or(header.alg);
    let mut validation = Validation::new(algorithm);
    validation.set_audience(&[oidc.client_id.as_str()]);
    validation.set_issuer(&[metadata.issuer.as_str()]);
    validation.validate_exp = true;

    let claims = decode::<Value>(id_token, &decoding_key, &validation)
        .map_err(|error| AppError::Unauthorized(format!("id token rejected: {error}")))?
        .claims;

    let nonce = claims.get("nonce").and_then(Value::as_str).unwrap_or("");
    if nonce != expected_nonce {
        return Err(AppError::Unauthorized(
            "id token nonce did not match this sign-in request".to_string(),
        ));
    }

    let subject = claims
        .get("sub")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Unauthorized("id token has no subject".to_string()))?
        .to_string();

    // Preference order mirrors what providers actually populate reliably.
    let username = ["preferred_username", "email", "sub"]
        .iter()
        .filter_map(|claim| claims.get(*claim).and_then(Value::as_str))
        .find(|value| !value.is_empty())
        .map(normalize_username)
        .ok_or_else(|| AppError::Unauthorized("id token has no usable username".to_string()))?;

    Ok(IdentityClaims {
        subject,
        username,
        display_name: claims
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
        groups: oidc
            .groups_claim
            .as_deref()
            .and_then(|claim| claims.get(claim))
            .map(collect_strings)
            .unwrap_or_default(),
    })
}

/// Links or creates the local user, then issues an OAD session.
///
/// Matching is by `(issuer, subject)` first, because a provider may rename a
/// user's email or username at any time without that being a different person.
async fn provision_user(
    state: &AppState,
    oidc: &OidcConfig,
    claims: &IdentityClaims,
) -> AppResult<LoginResponse> {
    let is_admin_by_group = oidc
        .admin_group
        .as_ref()
        .is_some_and(|group| claims.groups.iter().any(|value| value == group));

    let mut tx = state.db.begin().await?;
    let existing = sqlx::query(
        r#"
        SELECT id, username, display_name, is_admin
        FROM users
        WHERE oidc_issuer = $1 AND oidc_subject = $2
        "#,
    )
    .bind(&oidc.issuer)
    .bind(&claims.subject)
    .fetch_optional(&mut *tx)
    .await?;

    let row = match existing {
        Some(row) => {
            // Admin rights follow the directory when a group governs them, but a
            // locally granted admin is never silently demoted.
            let user_id: Uuid = row.get("id");
            sqlx::query(
                r#"
                UPDATE users
                SET display_name = COALESCE($2, display_name),
                    is_admin = is_admin OR $3,
                    updated_at = now()
                WHERE id = $1
                RETURNING id, username, display_name, is_admin
                "#,
            )
            .bind(user_id)
            .bind(claims.display_name.as_deref())
            .bind(is_admin_by_group)
            .fetch_one(&mut *tx)
            .await?
        }
        None => {
            // Claiming an existing password account by username would let anyone
            // who can create that username at the provider take it over.
            let taken: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM users WHERE username = $1 AND oidc_subject IS NULL",
            )
            .bind(&claims.username)
            .fetch_optional(&mut *tx)
            .await?;
            if taken.is_some() {
                return Err(AppError::conflict(format!(
                    "a local account named {} already exists; an administrator must link it",
                    claims.username
                )));
            }
            sqlx::query(
                r#"
                INSERT INTO users (username, display_name, is_admin, oidc_issuer, oidc_subject)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id, username, display_name, is_admin
                "#,
            )
            .bind(&claims.username)
            .bind(claims.display_name.as_deref())
            .bind(is_admin_by_group)
            .bind(&oidc.issuer)
            .bind(&claims.subject)
            .fetch_one(&mut *tx)
            .await?
        }
    };

    let user_id: Uuid = row.get("id");
    if oidc.groups_claim.is_some() {
        sync_groups(&mut tx, user_id, &claims.groups).await?;
    }
    tx.commit().await?;

    let auth_user = AuthUser {
        user_id,
        username: row.get("username"),
        is_admin: row.get("is_admin"),
    };
    let token = state.jwt.encode(&auth_user)?;
    audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: Some(user_id),
            ..audit::AuditEvent::new(
                "login_sso",
                serde_json::json!({
                    "username": auth_user.username,
                    "issuer": oidc.issuer,
                    "groups": claims.groups,
                }),
            )
        },
    )
    .await?;

    Ok(LoginResponse {
        token,
        user: UserResponse {
            id: user_id,
            username: auth_user.username,
            display_name: row.get("display_name"),
            is_admin: auth_user.is_admin,
        },
    })
}

/// Replaces the user's directory-derived group memberships.
///
/// Only groups carrying a matching `external_id` are touched, so groups a studio
/// manages by hand inside OAD are never removed by a directory login.
async fn sync_groups(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    groups: &[String],
) -> AppResult<()> {
    sqlx::query(
        r#"
        DELETE FROM user_groups
        WHERE user_id = $1
          AND group_id IN (SELECT id FROM groups WHERE external_id IS NOT NULL)
        "#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    if groups.is_empty() {
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO user_groups (user_id, group_id)
        SELECT $1, id FROM groups WHERE external_id = ANY($2)
        ON CONFLICT (user_id, group_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(groups)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Fetches and caches the discovery document and signing keys.
async fn provider_metadata(
    state: &AppState,
    oidc: &OidcConfig,
) -> AppResult<(ProviderMetadata, Vec<Jwk>)> {
    if let Some((metadata, keys, fetched_at)) = state
        .oidc
        .metadata
        .lock()
        .map_err(|_| AppError::internal("oidc metadata is poisoned"))?
        .as_ref()
    {
        if fetched_at.elapsed() < METADATA_TTL {
            return Ok((metadata.clone(), keys.clone()));
        }
    }

    let client = http_client()?;
    let discovery_url = format!("{}/.well-known/openid-configuration", oidc.issuer);
    let metadata: ProviderMetadata = client
        .get(&discovery_url)
        .send()
        .await
        .map_err(|error| {
            AppError::Configuration(format!("could not reach the identity provider: {error}"))
        })?
        .error_for_status()
        .map_err(|error| {
            AppError::Configuration(format!("identity provider discovery failed: {error}"))
        })?
        .json()
        .await
        .map_err(|error| {
            AppError::Configuration(format!("identity provider metadata invalid: {error}"))
        })?;

    // A mismatch means the document does not belong to the configured issuer,
    // which would let a redirect host impersonate it.
    if metadata.issuer.trim_end_matches('/') != oidc.issuer {
        return Err(AppError::Configuration(format!(
            "identity provider reports issuer {} but OAD_OIDC_ISSUER is {}",
            metadata.issuer, oidc.issuer
        )));
    }

    let jwks: JwksResponse = client
        .get(&metadata.jwks_uri)
        .send()
        .await
        .map_err(|error| AppError::Configuration(format!("could not fetch signing keys: {error}")))?
        .error_for_status()
        .map_err(|error| AppError::Configuration(format!("signing key fetch failed: {error}")))?
        .json()
        .await
        .map_err(|error| AppError::Configuration(format!("signing keys invalid: {error}")))?;

    *state
        .oidc
        .metadata
        .lock()
        .map_err(|_| AppError::internal("oidc metadata is poisoned"))? =
        Some((metadata.clone(), jwks.keys.clone(), Instant::now()));
    Ok((metadata, jwks.keys))
}

fn decoding_key(key: &Jwk) -> AppResult<DecodingKey> {
    match key.key_type.as_str() {
        "RSA" => {
            let (n, e) = key
                .n
                .as_deref()
                .zip(key.e.as_deref())
                .ok_or_else(|| AppError::Unauthorized("RSA key is incomplete".to_string()))?;
            DecodingKey::from_rsa_components(n, e)
                .map_err(|error| AppError::Unauthorized(format!("RSA key invalid: {error}")))
        }
        "EC" => {
            let (x, y) = key
                .x
                .as_deref()
                .zip(key.y.as_deref())
                .ok_or_else(|| AppError::Unauthorized("EC key is incomplete".to_string()))?;
            if key.crv.as_deref() != Some("P-256") {
                return Err(AppError::Unauthorized(
                    "only P-256 elliptic-curve keys are supported".to_string(),
                ));
            }
            DecodingKey::from_ec_components(x, y)
                .map_err(|error| AppError::Unauthorized(format!("EC key invalid: {error}")))
        }
        other => Err(AppError::Unauthorized(format!(
            "unsupported signing key type {other}"
        ))),
    }
}

fn require_oidc(state: &AppState) -> AppResult<&OidcConfig> {
    state.config.oidc.as_ref().ok_or_else(|| {
        AppError::Forbidden("single sign-on is not configured on this server".to_string())
    })
}

fn http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| AppError::internal(format!("could not build HTTP client: {error}")))
}

/// Drops expired authorizations so an idle server does not accumulate state.
fn prune_pending(pending: &mut HashMap<String, PendingAuth>) {
    pending.retain(|_, entry| entry.created_at.elapsed() < STATE_TTL);
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn code_challenge(verifier: &str) -> String {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Percent-encodes a query parameter value.
fn urlencode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push_str("%20"),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Trims a claim into a username the rest of the system accepts.
fn normalize_username(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => character,
            '@' => '.',
            _ => '-',
        })
        .collect()
}

/// Reads a groups claim, which providers send as either a list or one string.
fn collect_strings(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Value::String(single) => vec![single.clone()],
        _ => Vec::new(),
    }
}

/// Derives a display label from the issuer host, avoiding a hard-coded vendor.
fn provider_label(issuer: &str) -> String {
    issuer
        .split("://")
        .nth(1)
        .unwrap_or(issuer)
        .split('/')
        .next()
        .unwrap_or(issuer)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_from_email_claims_are_normalized() {
        assert_eq!(
            normalize_username("Ada.Lovelace@Studio.test"),
            "ada.lovelace.studio.test"
        );
        assert_eq!(normalize_username(" Lighting TD "), "lighting-td");
    }

    #[test]
    fn group_claims_accept_a_list_or_a_single_string() {
        assert_eq!(
            collect_strings(&serde_json::json!(["lighting", "layout"])),
            vec!["lighting".to_string(), "layout".to_string()]
        );
        assert_eq!(
            collect_strings(&serde_json::json!("lighting")),
            vec!["lighting".to_string()]
        );
        assert!(collect_strings(&serde_json::json!(7)).is_empty());
    }

    #[test]
    fn pkce_challenge_matches_the_rfc_7636_example() {
        // Vector from RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            code_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn expired_authorizations_are_pruned() {
        let mut pending = HashMap::new();
        pending.insert(
            "fresh".to_string(),
            PendingAuth {
                code_verifier: "v".into(),
                nonce: "n".into(),
                created_at: Instant::now(),
            },
        );
        pending.insert(
            "stale".to_string(),
            PendingAuth {
                code_verifier: "v".into(),
                nonce: "n".into(),
                created_at: Instant::now() - STATE_TTL - Duration::from_secs(1),
            },
        );
        prune_pending(&mut pending);
        assert!(pending.contains_key("fresh"));
        assert!(!pending.contains_key("stale"));
    }

    #[test]
    fn query_values_are_percent_encoded() {
        assert_eq!(urlencode("openid profile"), "openid%20profile");
        assert_eq!(urlencode("a/b?c=d"), "a%2Fb%3Fc%3Dd");
    }
}
