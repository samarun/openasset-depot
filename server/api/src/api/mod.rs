use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderMap, HeaderName, Method, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::RwLock;
use tower_http::{
    catch_panic::CatchPanicLayer,
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
    LatencyUnit,
};

use crate::{
    adapters::AdapterRegistry,
    auth::{oidc::OidcStore, AuthUser, JwtService},
    config::Config,
    error::{AppError, AppResult},
    filetypes::FileTypeMatcher,
    storage::LocalObjectStore,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: PgPool,
    pub storage: Arc<LocalObjectStore>,
    pub storage_maintenance: Arc<RwLock<()>>,
    pub jwt: Arc<JwtService>,
    pub filetypes: Arc<RwLock<FileTypeMatcher>>,
    pub adapters: Arc<AdapterRegistry>,
    /// Pending SSO authorizations and cached provider metadata.
    pub oidc: Arc<OidcStore>,
}

impl AppState {
    pub fn new(
        config: Config,
        db: PgPool,
        storage: LocalObjectStore,
        filetypes: FileTypeMatcher,
    ) -> Self {
        let jwt = JwtService::new(config.jwt_secret.clone(), config.jwt_ttl_seconds);
        Self {
            config,
            db,
            storage: Arc::new(storage),
            storage_maintenance: Arc::new(RwLock::new(())),
            jwt: Arc::new(jwt),
            filetypes: Arc::new(RwLock::new(filetypes)),
            adapters: Arc::new(AdapterRegistry::default_registry()),
            oidc: Arc::new(OidcStore::default()),
        }
    }

    pub fn require_user(&self, headers: &HeaderMap) -> AppResult<AuthUser> {
        let value = headers
            .get(axum::http::header::AUTHORIZATION)
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".to_string()))?
            .to_str()
            .map_err(|_| AppError::Unauthorized("invalid Authorization header".to_string()))?;
        let token = value.strip_prefix("Bearer ").ok_or_else(|| {
            AppError::Unauthorized("Authorization must use Bearer token".to_string())
        })?;
        self.jwt.decode(token)
    }
}

pub fn build_router(state: AppState) -> Router {
    let request_id_header = axum::http::HeaderName::from_static("x-request-id");
    let allowed_origins = state
        .config
        .cors_allowed_origins
        .iter()
        .map(|origin| {
            origin
                .parse()
                .expect("CORS origins are validated at startup")
        })
        .collect::<Vec<_>>();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::HEAD,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::RANGE,
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("x-upload-offset"),
        ])
        .expose_headers([
            HeaderName::from_static("x-request-id"),
            header::ACCEPT_RANGES,
            header::CONTENT_RANGE,
            header::CONTENT_LENGTH,
        ])
        .allow_private_network(true);

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/users", post(crate::auth::create_user))
        .route("/api/auth/login", post(crate::auth::login))
        .route(
            "/api/auth/signup",
            get(crate::auth::signup_status).post(crate::auth::signup),
        )
        .route(
            "/api/auth/change-password",
            post(crate::auth::change_password),
        )
        .route(
            "/api/auth/sso",
            get(crate::auth::oidc::oidc_status).post(crate::auth::oidc::oidc_start),
        )
        .route(
            "/api/auth/sso/callback",
            post(crate::auth::oidc::oidc_callback),
        )
        .route(
            "/api/depots",
            get(crate::depot::list_depots).post(crate::depot::create_depot),
        )
        .route(
            "/api/depots/:id/permissions",
            get(crate::permissions::list_depot_permissions)
                .post(crate::permissions::grant_depot_permission),
        )
        .route(
            "/api/depots/:id/group-permissions",
            get(crate::groups::list_group_permissions).post(crate::groups::grant_group_permission),
        )
        .route(
            "/api/depots/:id/group-permissions/:group_id",
            delete(crate::groups::revoke_group_permission),
        )
        .route(
            "/api/groups",
            get(crate::groups::list_groups).post(crate::groups::create_group),
        )
        .route(
            "/api/groups/:id/members",
            get(crate::groups::list_group_members).post(crate::groups::add_group_member),
        )
        .route(
            "/api/groups/:id/members/:user_id",
            delete(crate::groups::remove_group_member),
        )
        .route(
            "/api/streams",
            get(crate::streams::list_streams).post(crate::streams::create_stream),
        )
        .route(
            "/api/workspaces",
            get(crate::workspaces::list_workspaces).post(crate::workspaces::create_workspace),
        )
        .route(
            "/api/workspaces/:id",
            delete(crate::workspaces::delete_workspace),
        )
        .route(
            "/api/workspaces/:id/collaborators",
            get(crate::workspaces::list_collaborators),
        )
        .route("/api/files/add", post(crate::changelists::file_add))
        .route("/api/files/edit", post(crate::changelists::file_edit))
        .route("/api/files/delete", post(crate::changelists::file_delete))
        .route("/api/files/revert", post(crate::changelists::file_revert))
        .route("/api/files/lock", post(crate::locking::lock_file))
        .route("/api/files/unlock", post(crate::locking::unlock_file))
        .route(
            "/api/changelists",
            post(crate::changelists::create_changelist),
        )
        .route(
            "/api/changelists/:id/submit",
            post(crate::changelists::submit_changelist),
        )
        .route(
            "/api/changelists/:id/shelve",
            get(crate::shelves::get_shelf)
                .post(crate::shelves::shelve_changelist)
                .delete(crate::shelves::discard_shelf),
        )
        .route(
            "/api/changelists/:id/unshelve",
            post(crate::shelves::unshelve_changelist),
        )
        .route(
            "/api/dependencies/impact",
            get(crate::dependencies::dependency_impact),
        )
        .route("/api/shelves", get(crate::shelves::list_shelves))
        .route(
            "/api/shelves/content",
            get(crate::shelves::download_shelf_content),
        )
        .route("/api/uploads", post(crate::uploads::begin_upload))
        .route(
            "/api/uploads/:id",
            get(crate::uploads::get_upload).post(crate::uploads::upload_chunk),
        )
        .route(
            "/api/uploads/:id/finalize",
            post(crate::uploads::finalize_upload),
        )
        .route("/api/sync/plan", post(crate::sync::plan_sync))
        .route("/api/sync/ack", post(crate::sync::ack_sync))
        .route("/api/sync/download", post(crate::sync::download_file))
        .route("/api/files/history", get(crate::sync::file_history))
        .route(
            "/api/files/preview",
            get(crate::previews::download_preview).post(crate::previews::upload_preview),
        )
        .route(
            "/api/reviews/comments",
            get(crate::reviews::list_comments).post(crate::reviews::create_comment),
        )
        .route(
            "/api/reviews/comments/:id/resolve",
            post(crate::reviews::resolve_comment),
        )
        .route(
            "/api/reviews/media",
            get(crate::reviews::download_review_media),
        )
        .route(
            "/api/reviews/proxy",
            get(crate::reviews::download_review_proxy).post(crate::reviews::upload_review_proxy),
        )
        .route(
            "/api/reviews/requests",
            get(crate::reviews::requests::list_review_requests)
                .post(crate::reviews::requests::create_review_request),
        )
        .route(
            "/api/reviews/requests/:id",
            get(crate::reviews::requests::get_review_request),
        )
        .route(
            "/api/reviews/requests/:id/decision",
            post(crate::reviews::requests::decide_review_request),
        )
        .route(
            "/api/reviews/requests/:id/close",
            post(crate::reviews::requests::close_review_request),
        )
        .route("/api/locks", get(crate::locking::list_locks))
        .route("/api/locks/page", get(crate::locking::list_locks_page))
        .route(
            "/api/admin/locks/force-unlock",
            post(crate::locking::force_unlock_file),
        )
        .route("/api/audit", get(crate::audit::list_audit_events))
        .route(
            "/api/dependencies",
            get(crate::dependencies::list_dependencies),
        )
        .route("/api/filetypes", get(crate::filetypes::list_filetypes))
        .route(
            "/api/filetypes/match",
            post(crate::filetypes::match_filetype),
        )
        .route("/api/validate", post(crate::validation::validate_paths))
        .route(
            "/api/admin/storage/verify",
            get(crate::admin::verify_storage),
        )
        .route(
            "/api/admin/storage/cleanup",
            post(crate::admin::cleanup_storage),
        )
        .route("/api/adapters", get(crate::adapters::list_adapters))
        .route(
            "/api/adapters/detect",
            post(crate::adapters::detect_project),
        )
        .route(
            "/api/adapters/scan",
            post(crate::adapters::scan_dependencies),
        )
        .route(
            "/api/adapters/preview",
            post(crate::adapters::generate_preview),
        )
        .route(
            "/api/adapters/metadata",
            post(crate::adapters::extract_metadata),
        )
        .route(
            "/api/adapters/validate",
            post(crate::adapters::validate_adapter_changelist),
        )
        .with_state(state.clone())
        .layer(cors)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(tower_http::trace::DefaultMakeSpan::new().include_headers(false))
                .on_response(
                    tower_http::trace::DefaultOnResponse::new().latency_unit(LatencyUnit::Millis),
                ),
        )
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            state.config.request_timeout,
        ))
        .layer(CatchPanicLayer::new())
        .layer(DefaultBodyLimit::max(state.config.max_upload_bytes))
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> (StatusCode, Json<HealthResponse>) {
    (StatusCode::OK, Json(HealthResponse { status: "ok" }))
}

async fn ready(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> AppResult<Json<HealthResponse>> {
    sqlx::query_scalar::<_, i64>("SELECT 1::bigint")
        .fetch_one(&state.db)
        .await?;
    Ok(Json(HealthResponse { status: "ready" }))
}
