use std::time::Duration;

use tokio::task::JoinHandle;

use crate::{admin, api::AppState, audit};

/// Grace period before the first sweep, so a restarting server finishes serving
/// traffic before it starts reading every recent blob back off disk.
const STARTUP_DELAY: Duration = Duration::from_secs(60);

/// Objects must be unreferenced for this long before cleanup will remove them.
/// This is deliberately generous: an in-flight submit writes chunks before the
/// database rows that reference them exist.
const CLEANUP_MIN_AGE_HOURS: u64 = 24;

/// Upper bound on deletions per sweep, so a misbehaving run stays recoverable.
const CLEANUP_MAX_DELETE: usize = 5_000;

/// How long an unfinished upload session may sit idle before it is abandoned.
///
/// Long enough that an artist can resume after an overnight network outage,
/// short enough that staged bytes from dead sessions are not kept forever.
const UPLOAD_SESSION_MAX_IDLE_HOURS: i64 = 48;

/// Starts the periodic storage integrity and orphan-cleanup sweep.
///
/// Returns `None` when `OAD_INTEGRITY_INTERVAL_SECONDS` is zero, which is the
/// default: studios opt in rather than silently paying for background I/O. Only
/// one replica should enable it, since sweeps take a storage-wide lease.
pub fn spawn_integrity_worker(state: AppState) -> Option<JoinHandle<()>> {
    let interval = state.config.integrity_interval;
    if interval.is_zero() {
        tracing::info!("scheduled integrity worker disabled");
        return None;
    }
    let sample_size = state.config.integrity_sample_size as i64;
    tracing::info!(
        interval_secs = interval.as_secs(),
        sample_size,
        "scheduled integrity worker enabled"
    );

    Some(tokio::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        let mut ticker = tokio::time::interval(interval);
        // A slow sweep must not cause a burst of catch-up runs afterwards.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            run_sweep(&state, sample_size).await;
        }
    }))
}

async fn run_sweep(state: &AppState, sample_size: i64) {
    match admin::run_verification(state, sample_size).await {
        Ok(summary) => {
            if summary.failed > 0 {
                tracing::error!(
                    checked = summary.checked,
                    failed = summary.failed,
                    "scheduled integrity sweep found corrupt blobs"
                );
            } else {
                tracing::info!(
                    checked = summary.checked,
                    "scheduled integrity sweep found no corruption"
                );
            }
            record(
                state,
                "storage_verify_scheduled",
                serde_json::json!({
                    "checked": summary.checked,
                    "failed": summary.failed,
                    "failures": summary
                        .failures
                        .iter()
                        .map(|report| report.blob_hash.clone())
                        .collect::<Vec<_>>(),
                }),
            )
            .await;
        }
        Err(error) => tracing::error!(%error, "scheduled integrity sweep failed"),
    }

    match admin::run_cleanup(state, false, CLEANUP_MIN_AGE_HOURS, CLEANUP_MAX_DELETE).await {
        Ok(summary) => {
            tracing::info!(
                scanned = summary.scanned,
                orphaned = summary.orphaned,
                deleted = summary.deleted,
                deleted_bytes = summary.deleted_bytes,
                "scheduled orphan cleanup complete"
            );
            if summary.deleted > 0 {
                record(
                    state,
                    "storage_cleanup_scheduled",
                    serde_json::json!({
                        "scanned": summary.scanned,
                        "orphaned": summary.orphaned,
                        "deleted": summary.deleted,
                        "deleted_bytes": summary.deleted_bytes,
                    }),
                )
                .await;
            }
        }
        Err(error) => tracing::error!(%error, "scheduled orphan cleanup failed"),
    }

    match expire_upload_sessions(state).await {
        Ok(0) => {}
        Ok(expired) => {
            tracing::info!(expired, "reclaimed abandoned upload sessions");
            record(
                state,
                "upload_sessions_expired",
                serde_json::json!({ "expired": expired }),
            )
            .await;
        }
        Err(error) => tracing::error!(%error, "upload session expiry failed"),
    }
}

/// Drops unfinished upload sessions and their staged bytes.
///
/// The row is deleted only after the staging file is gone, so a failure here
/// leaves a resumable session rather than an orphaned file with no record.
async fn expire_upload_sessions(state: &AppState) -> crate::error::AppResult<usize> {
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(UPLOAD_SESSION_MAX_IDLE_HOURS);
    let rows = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM upload_sessions WHERE finalized_at IS NULL AND updated_at < $1 LIMIT 1000",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await?;

    let mut expired = 0usize;
    for upload_id in rows {
        let staging = state.storage.temp_upload_path(upload_id);
        match tokio::fs::remove_file(&staging).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(%error, %upload_id, "could not remove staged upload");
                continue;
            }
        }
        sqlx::query("DELETE FROM upload_sessions WHERE id = $1 AND finalized_at IS NULL")
            .bind(upload_id)
            .execute(&state.db)
            .await?;
        expired += 1;
    }
    Ok(expired)
}

/// Records a sweep result with no actor, marking it as system-initiated.
async fn record(state: &AppState, action: &str, detail: serde_json::Value) {
    if let Err(error) = audit::record(
        &state.db,
        audit::AuditEvent {
            actor_user_id: None,
            ..audit::AuditEvent::new(action, detail)
        },
    )
    .await
    {
        tracing::warn!(%error, action, "failed to record scheduled maintenance audit event");
    }
}
