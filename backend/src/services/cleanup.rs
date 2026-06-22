use std::time::{Duration, SystemTime};

use crate::{
    error::AppResult,
    services::{problems::problem_exists, storage},
    state::AppState,
};

/// How often the background reaper sweeps for orphaned problem-upload drafts.
const REAP_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
/// A draft must be older than this before it is reaped, so in-flight uploads
/// and a form the user is still editing are never disturbed.
const REAP_GRACE: Duration = Duration::from_secs(24 * 60 * 60);

/// Spawn the orphan-draft reaper as a background task. This is the兜底 for
/// file-integrity: the frontend's unmount side-effect (`DELETE .../cleanup` via
/// `navigator.sendBeacon`) is best-effort and unreliable (the tab can close
/// mid-flight, the beacon can be dropped). The reaper guarantees that files
/// uploaded for a problem UUID that was never persisted are eventually
/// reclaimed, without ever touching a saved problem's assets.
///
/// Runs in-process, so it covers the Docker Compose deployment where Vercel
/// cron cannot reach the backend.
pub fn spawn_reaper(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(REAP_INTERVAL);
        loop {
            // The first tick completes immediately, so a freshly-restarted
            // server reaps stale orphans left behind by a previous run
            // (still gated by the grace window) without waiting a full interval.
            ticker.tick().await;
            match reap_orphan_drafts(&state).await {
                Ok(removed) if removed > 0 => {
                    tracing::info!(removed, "reaped orphan problem draft dirs");
                }
                Ok(_) => {}
                Err(err) => tracing::warn!(error = %err, "orphan draft reap failed"),
            }
        }
    });
}

/// Scan every problem-upload draft directory and delete the ones that are both
/// stale (older than the grace window) and orphaned (no matching `problems`
/// row). Returns the number of directories removed.
async fn reap_orphan_drafts(state: &AppState) -> AppResult<u64> {
    let drafts = storage::list_problem_draft_dirs(state).await?;
    let now = SystemTime::now();
    let mut removed = 0u64;
    for draft in drafts {
        if now.duration_since(draft.modified).unwrap_or(Duration::ZERO) < REAP_GRACE {
            continue;
        }
        if problem_exists(&state.pool, draft.user_id, draft.problem_id).await? {
            continue;
        }
        if storage::delete_problem_draft_dir(state, draft.user_id, draft.problem_id)
            .await
            .is_ok()
        {
            removed += 1;
        }
    }
    Ok(removed)
}
