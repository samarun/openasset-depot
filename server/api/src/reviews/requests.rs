//! Review requests: asking named people to sign off on one asset revision.
//!
//! A request targets an immutable `file_revisions` row, never a workspace file,
//! so an approval always names exactly the bytes that were approved. Resubmitting
//! the asset creates a new revision and therefore needs a new review, which is
//! the property that makes an approval worth recording.

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    api::AppState,
    audit,
    error::{AppError, AppResult},
    idempotency,
    paths::normalize_depot_path,
    permissions::{ensure_depot_permission, DepotPermission},
    workspaces::workspace_for_user,
};

use super::revision_for_target;

/// Cap on reviewers per request. A sign-off list longer than this is a meeting,
/// not a review, and it keeps the fan-out of notification work bounded.
const MAX_REVIEWERS: usize = 20;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateReviewRequestBody {
    pub workspace_id: Uuid,
    pub path: String,
    pub revision_number: i32,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub reviewers: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewDecisionBody {
    pub workspace_id: Uuid,
    /// `approved` or `changes_requested`.
    pub decision: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewRequestListQuery {
    pub workspace_id: Uuid,
    /// Optional exact state filter, for example `open`.
    pub state: Option<String>,
    /// When true, only requests where the caller still owes a decision.
    #[serde(default)]
    pub awaiting_me: bool,
}

#[derive(Debug, Deserialize)]
pub struct ReviewRequestQuery {
    pub workspace_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewerResponse {
    pub reviewer_user_id: Uuid,
    pub reviewer: String,
    pub decision: String,
    pub note: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewRequestResponse {
    pub id: Uuid,
    pub path: String,
    pub revision_number: i32,
    pub requested_by: Uuid,
    pub requester: String,
    pub title: String,
    pub description: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub reviewers: Vec<ReviewerResponse>,
}

/// Opens a review on a submitted revision and assigns its reviewers.
pub async fn create_review_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateReviewRequestBody>,
) -> AppResult<Json<ReviewRequestResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, request.workspace_id, &user).await?;
    let path = normalize_depot_path(&request.path)?;
    let title = request.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::bad_request(
            "review title must contain between 1 and 200 characters",
        ));
    }
    let description = request.description.trim().to_string();
    if description.chars().count() > 4_000 {
        return Err(AppError::bad_request(
            "review description cannot exceed 4000 characters",
        ));
    }

    let mut reviewers = request.reviewers.clone();
    reviewers.sort();
    reviewers.dedup();
    if reviewers.is_empty() {
        return Err(AppError::bad_request(
            "a review needs at least one reviewer",
        ));
    }
    if reviewers.len() > MAX_REVIEWERS {
        return Err(AppError::bad_request(format!(
            "a review cannot have more than {MAX_REVIEWERS} reviewers"
        )));
    }

    // Assigning someone who cannot open the asset would produce a review that can
    // never complete, so this is rejected up front rather than at decision time.
    for reviewer_id in &reviewers {
        // The admin flag has to come from the row, not be assumed: a system admin
        // reaches every depot without an explicit grant, and treating them as an
        // ordinary user here would reject the most common approver on a small team.
        let is_admin: Option<bool> = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = $1")
            .bind(reviewer_id)
            .fetch_optional(&state.db)
            .await?;
        let is_admin = is_admin.ok_or_else(|| {
            AppError::bad_request(format!("reviewer {reviewer_id} does not exist"))
        })?;
        let reviewer = crate::auth::AuthUser {
            user_id: *reviewer_id,
            username: String::new(),
            is_admin,
        };
        ensure_depot_permission(
            &state.db,
            &reviewer,
            workspace.depot_id,
            DepotPermission::Read,
        )
        .await
        .map_err(|_| {
            AppError::bad_request(format!(
                "reviewer {reviewer_id} cannot read this depot and would block the review"
            ))
        })?;
    }

    let revision_id = revision_for_target(
        &state.db,
        workspace.stream_id,
        &path,
        request.revision_number,
    )
    .await?;

    let idempotency_request = serde_json::json!({
        "revision_id": revision_id,
        "title": title,
        "description": description,
        "reviewers": reviewers,
    });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "review_request_create",
        &idempotency_request,
        || async {
            let mut tx = state.db.begin().await?;
            let request_id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO review_requests
                    (revision_id, stream_id, requested_by, title, description)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id
                "#,
            )
            .bind(revision_id)
            .bind(workspace.stream_id)
            .bind(user.user_id)
            .bind(&title)
            .bind(&description)
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| match &error {
                sqlx::Error::Database(db_error) if db_error.is_unique_violation() => {
                    AppError::conflict("this revision already has an open review")
                }
                _ => AppError::Database(error),
            })?;

            for reviewer_id in &reviewers {
                sqlx::query(
                    "INSERT INTO review_request_reviewers (review_request_id, reviewer_user_id) VALUES ($1, $2)",
                )
                .bind(request_id)
                .bind(reviewer_id)
                .execute(&mut *tx)
                .await?;
            }

            audit::record_tx(
                &mut tx,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(workspace.stream_id),
                    workspace_id: Some(workspace.id),
                    depot_path: Some(&path),
                    ..audit::AuditEvent::new(
                        "review_request_create",
                        serde_json::json!({
                            "review_request_id": request_id,
                            "revision_number": request.revision_number,
                            "reviewer_count": reviewers.len(),
                        }),
                    )
                },
            )
            .await?;
            tx.commit().await?;

            request_by_id(&state.db, request_id).await
        },
    )
    .await?;

    Ok(Json(response))
}

/// Lists reviews on the caller's stream, newest first.
pub async fn list_review_requests(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReviewRequestListQuery>,
) -> AppResult<Json<Vec<ReviewRequestResponse>>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    if let Some(state_filter) = query.state.as_deref() {
        if !matches!(
            state_filter,
            "open" | "approved" | "changes_requested" | "closed"
        ) {
            return Err(AppError::bad_request(
                "state must be open, approved, changes_requested, or closed",
            ));
        }
    }

    let rows = sqlx::query(
        r#"
        SELECT rr.id
        FROM review_requests rr
        WHERE rr.stream_id = $1
          AND ($2::text IS NULL OR rr.state = $2)
          AND (
            NOT $3::bool
            OR EXISTS (
                SELECT 1 FROM review_request_reviewers rv
                WHERE rv.review_request_id = rr.id
                  AND rv.reviewer_user_id = $4
                  AND rv.decision = 'pending'
            )
          )
        ORDER BY rr.created_at DESC, rr.id DESC
        LIMIT 200
        "#,
    )
    .bind(workspace.stream_id)
    .bind(query.state.as_deref())
    .bind(query.awaiting_me)
    .bind(user.user_id)
    .fetch_all(&state.db)
    .await?;

    let mut requests = Vec::with_capacity(rows.len());
    for row in rows {
        requests.push(request_by_id(&state.db, row.get("id")).await?);
    }
    Ok(Json(requests))
}

/// Reads one review with its reviewer decisions.
pub async fn get_review_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(request_id): AxumPath<Uuid>,
    Query(query): Query<ReviewRequestQuery>,
) -> AppResult<Json<ReviewRequestResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    ensure_request_on_stream(&state.db, request_id, workspace.stream_id).await?;
    Ok(Json(request_by_id(&state.db, request_id).await?))
}

/// Records one reviewer's decision and recomputes the request's state.
pub async fn decide_review_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(request_id): AxumPath<Uuid>,
    Json(body): Json<ReviewDecisionBody>,
) -> AppResult<Json<ReviewRequestResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, body.workspace_id, &user).await?;
    ensure_request_on_stream(&state.db, request_id, workspace.stream_id).await?;

    let decision = body.decision.trim();
    if !matches!(decision, "approved" | "changes_requested") {
        return Err(AppError::bad_request(
            "decision must be approved or changes_requested",
        ));
    }
    let note = body
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    if note.is_some_and(|n| n.chars().count() > 2_000) {
        return Err(AppError::bad_request(
            "review note cannot exceed 2000 characters",
        ));
    }

    let current_state: String =
        sqlx::query_scalar("SELECT state FROM review_requests WHERE id = $1")
            .bind(request_id)
            .fetch_one(&state.db)
            .await?;
    if current_state == "closed" {
        return Err(AppError::conflict("this review is closed"));
    }

    let idempotency_request = serde_json::json!({
        "review_request_id": request_id,
        "decision": decision,
        "note": note,
    });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "review_request_decision",
        &idempotency_request,
        || async {
            let mut tx = state.db.begin().await?;
            let updated = sqlx::query(
                r#"
                UPDATE review_request_reviewers
                SET decision = $3, note = $4, decided_at = now()
                WHERE review_request_id = $1 AND reviewer_user_id = $2
                "#,
            )
            .bind(request_id)
            .bind(user.user_id)
            .bind(decision)
            .bind(note)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if updated == 0 {
                return Err(AppError::Forbidden(
                    "only an assigned reviewer can decide this review".to_string(),
                ));
            }

            let resolved_state = recompute_request_state(&mut tx, request_id).await?;

            audit::record_tx(
                &mut tx,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(workspace.stream_id),
                    workspace_id: Some(workspace.id),
                    ..audit::AuditEvent::new(
                        "review_request_decision",
                        serde_json::json!({
                            "review_request_id": request_id,
                            "decision": decision,
                            "review_state": resolved_state,
                        }),
                    )
                },
            )
            .await?;
            tx.commit().await?;

            request_by_id(&state.db, request_id).await
        },
    )
    .await?;

    Ok(Json(response))
}

/// Closes a review without a verdict. Requester or system admin only.
pub async fn close_review_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(request_id): AxumPath<Uuid>,
    Json(query): Json<ReviewRequestQuery>,
) -> AppResult<Json<ReviewRequestResponse>> {
    let user = state.require_user(&headers)?;
    let workspace = workspace_for_user(&state.db, query.workspace_id, &user).await?;
    let requested_by = ensure_request_on_stream(&state.db, request_id, workspace.stream_id).await?;
    if requested_by != user.user_id && !user.is_admin {
        return Err(AppError::Forbidden(
            "only the requester or an admin can close a review".to_string(),
        ));
    }

    let idempotency_request = serde_json::json!({ "review_request_id": request_id });
    let response = idempotency::run(
        &state.db,
        &headers,
        &user,
        "review_request_close",
        &idempotency_request,
        || async {
            let mut tx = state.db.begin().await?;
            let closed = sqlx::query(
                r#"
                UPDATE review_requests
                SET state = 'closed', closed_at = now(), closed_by = $2, updated_at = now()
                WHERE id = $1 AND state <> 'closed'
                "#,
            )
            .bind(request_id)
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if closed == 0 {
                return Err(AppError::conflict("this review is already closed"));
            }
            audit::record_tx(
                &mut tx,
                audit::AuditEvent {
                    actor_user_id: Some(user.user_id),
                    stream_id: Some(workspace.stream_id),
                    workspace_id: Some(workspace.id),
                    ..audit::AuditEvent::new(
                        "review_request_close",
                        serde_json::json!({ "review_request_id": request_id }),
                    )
                },
            )
            .await?;
            tx.commit().await?;
            request_by_id(&state.db, request_id).await
        },
    )
    .await?;

    Ok(Json(response))
}

/// Derives the request state from its reviewers' decisions.
///
/// A single `changes_requested` outranks any number of approvals: if one lead
/// says the asset is not ready, the review is not approved. Approval requires
/// every assigned reviewer to have said yes.
async fn recompute_request_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    request_id: Uuid,
) -> AppResult<String> {
    let row = sqlx::query(
        r#"
        SELECT count(*) FILTER (WHERE decision = 'pending') AS pending,
               count(*) FILTER (WHERE decision = 'changes_requested') AS rejected
        FROM review_request_reviewers
        WHERE review_request_id = $1
        "#,
    )
    .bind(request_id)
    .fetch_one(&mut **tx)
    .await?;
    let pending: i64 = row.get("pending");
    let rejected: i64 = row.get("rejected");

    let next_state = if rejected > 0 {
        "changes_requested"
    } else if pending == 0 {
        "approved"
    } else {
        "open"
    };

    sqlx::query("UPDATE review_requests SET state = $2, updated_at = now() WHERE id = $1")
        .bind(request_id)
        .bind(next_state)
        .execute(&mut **tx)
        .await?;
    Ok(next_state.to_string())
}

/// Confirms the review belongs to the caller's stream, returning its requester.
async fn ensure_request_on_stream(
    db: &PgPool,
    request_id: Uuid,
    stream_id: Uuid,
) -> AppResult<Uuid> {
    let row = sqlx::query("SELECT stream_id, requested_by FROM review_requests WHERE id = $1")
        .bind(request_id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("review request not found".to_string()))?;
    if row.get::<Uuid, _>("stream_id") != stream_id {
        return Err(AppError::Forbidden(
            "review request is outside this workspace stream".to_string(),
        ));
    }
    Ok(row.get("requested_by"))
}

async fn request_by_id(db: &PgPool, request_id: Uuid) -> AppResult<ReviewRequestResponse> {
    let row = sqlx::query(
        r#"
        SELECT rr.id, fr.depot_path, fr.revision_number, rr.requested_by,
               COALESCE(u.display_name, u.username) AS requester,
               rr.title, rr.description, rr.state, rr.created_at, rr.updated_at, rr.closed_at
        FROM review_requests rr
        JOIN file_revisions fr ON fr.id = rr.revision_id
        JOIN users u ON u.id = rr.requested_by
        WHERE rr.id = $1
        "#,
    )
    .bind(request_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::NotFound("review request not found".to_string()))?;

    let reviewers = sqlx::query(
        r#"
        SELECT rv.reviewer_user_id,
               COALESCE(u.display_name, u.username) AS reviewer,
               rv.decision, rv.note, rv.decided_at
        FROM review_request_reviewers rv
        JOIN users u ON u.id = rv.reviewer_user_id
        WHERE rv.review_request_id = $1
        ORDER BY u.username ASC
        "#,
    )
    .bind(request_id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|row| ReviewerResponse {
        reviewer_user_id: row.get("reviewer_user_id"),
        reviewer: row.get("reviewer"),
        decision: row.get("decision"),
        note: row.get("note"),
        decided_at: row.get("decided_at"),
    })
    .collect();

    Ok(ReviewRequestResponse {
        id: row.get("id"),
        path: row.get("depot_path"),
        revision_number: row.get("revision_number"),
        requested_by: row.get("requested_by"),
        requester: row.get("requester"),
        title: row.get("title"),
        description: row.get("description"),
        state: row.get("state"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        closed_at: row.get("closed_at"),
        reviewers,
    })
}
