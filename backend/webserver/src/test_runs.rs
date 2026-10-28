//! Test runs that have not been saved (Test View's "Run Test" without "Save
//! Test").
//!
//! A run makes the real HTTP call and keeps what came back here, in memory,
//! under a `run_id`. Nothing touches the database or the EQP folders until
//! `POST /test-view/save/test` promotes the run: that creates the endpoint if
//! it isn't there already, adds the QP and writes the History row.
//!
//! Runs that are never saved just age out. A run that timed out, or was
//! stopped, is dropped straight away.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::{events::ServerEvent, timer::TimerConfig};

/// Every run id starts with this, so a callback can tell a run from a saved
/// endpoint (real EIDs look like `E0001-AAA`) even after the run has expired.
pub const RUN_ID_PREFIX: &str = "run-";

/// How long an unsaved run stays available to Save.
const RUN_TTL: Duration = Duration::from_secs(30 * 60);
/// Most runs kept at once; the oldest finished ones go first.
const MAX_RUNS: usize = 200;

static NEXT: AtomicU64 = AtomicU64::new(1);

pub fn is_run_id(id: &str) -> bool {
    id.starts_with(RUN_ID_PREFIX)
}

fn new_run_id() -> String {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("{RUN_ID_PREFIX}{ms}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// What the HTTP call came back with.
#[derive(Debug, Clone)]
pub struct RunResult {
    pub status_code: i32,
    pub response_time_ms: i32,
    pub response_body: Value,
    pub request_headers: Value,
    pub response_headers: Value,
}

/// What a run was asked to do, kept so Save can write it.
#[derive(Debug, Clone)]
pub struct PendingRun {
    pub run_id: String,
    /// An existing endpoint this run re-tests, if the caller named one.
    pub endpoint_id: Option<String>,
    pub endpoint_str: String,
    pub method: String,
    pub annotation: Option<String>,
    pub request_body: Value,
    /// `None` while the call is still in flight.
    pub result: Option<RunResult>,
    started: Instant,
    timer: CancellationToken,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TakeError {
    /// Never existed, or timed out, was stopped, or aged out.
    NotFound,
    /// The HTTP call hasn't come back yet.
    StillRunning,
}

#[derive(Clone, Default)]
pub struct PendingRuns {
    inner: Arc<Mutex<HashMap<String, PendingRun>>>,
}

impl PendingRuns {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a run that is about to start and returns its id. The timer
    /// token is the one the run's countdown watches.
    pub fn begin(
        &self,
        endpoint_id: Option<String>,
        endpoint_str: String,
        method: String,
        annotation: Option<String>,
        request_body: Value,
        timer: CancellationToken,
    ) -> String {
        let run_id = new_run_id();
        let mut runs = self.inner.lock().unwrap();
        Self::purge(&mut runs);
        runs.insert(
            run_id.clone(),
            PendingRun {
                run_id: run_id.clone(),
                endpoint_id,
                endpoint_str,
                method,
                annotation,
                request_body,
                result: None,
                started: Instant::now(),
                timer,
            },
        );
        run_id
    }

    /// Records the outcome of a run still in flight and stops its countdown.
    /// `None` if the run is gone (it timed out, was stopped, or aged out).
    pub fn finish(&self, run_id: &str, result: RunResult) -> Option<PendingRun> {
        let mut runs = self.inner.lock().unwrap();
        let run = runs.get_mut(run_id)?;
        run.timer.cancel();
        run.result = Some(result);
        Some(run.clone())
    }

    /// Drops a run (timed out, or stopped) and stops its countdown. True if
    /// it was there.
    pub fn discard(&self, run_id: &str) -> bool {
        match self.inner.lock().unwrap().remove(run_id) {
            Some(run) => {
                run.timer.cancel();
                true
            }
            None => false,
        }
    }

    /// Removes a finished run so it can be saved. Put it back with
    /// `restore` if the save fails.
    pub fn take_finished(&self, run_id: &str) -> Result<PendingRun, TakeError> {
        let mut runs = self.inner.lock().unwrap();
        match runs.get(run_id) {
            None => Err(TakeError::NotFound),
            Some(run) if run.result.is_none() => Err(TakeError::StillRunning),
            Some(_) => Ok(runs.remove(run_id).expect("checked above")),
        }
    }

    pub fn restore(&self, run: PendingRun) {
        self.inner.lock().unwrap().insert(run.run_id.clone(), run);
    }

    fn purge(runs: &mut HashMap<String, PendingRun>) {
        runs.retain(|_, r| r.started.elapsed() < RUN_TTL);
        while runs.len() >= MAX_RUNS {
            let oldest = runs
                .values()
                .filter(|r| r.result.is_some())
                .min_by_key(|r| r.started)
                .map(|r| r.run_id.clone());
            match oldest {
                Some(id) => {
                    runs.remove(&id);
                }
                None => break,
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}

/// The countdown for an unsaved run. Same idea as `timer::spawn_timer`, but
/// it reports as `RunTick` / `RunTimeout` for the run id, so it never looks
/// like a saved endpoint's test to a client that is watching one. Cancelling
/// the returned token (done when the result arrives) stops it quietly.
pub fn spawn_run_timer(
    run_id: String,
    token: CancellationToken,
    config: TimerConfig,
    events_tx: broadcast::Sender<ServerEvent>,
    runs: PendingRuns,
) {
    tokio::spawn(async move {
        let start = Instant::now();
        let interval = Duration::from_millis(config.tick_interval_ms);
        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {}
                _ = token.cancelled() => return,
            }
            let elapsed = start.elapsed().as_millis() as u64;
            let _ = events_tx.send(ServerEvent::RunTick {
                run_id: run_id.clone(),
                elapsed_ms: elapsed,
                remaining_ms: config.limit_ms.saturating_sub(elapsed),
                limit_ms: config.limit_ms,
            });
            if elapsed >= config.limit_ms {
                // A result that landed during the last tick wins.
                if !token.is_cancelled() && runs.discard(&run_id) {
                    let _ = events_tx.send(ServerEvent::RunTimeout { run_id });
                }
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn begin(runs: &PendingRuns) -> (String, CancellationToken) {
        let token = CancellationToken::new();
        let id = runs.begin(None, "https://x.test/a".into(), "GET".into(), None, json!({}), token.clone());
        (id, token)
    }

    fn result() -> RunResult {
        RunResult {
            status_code: 200,
            response_time_ms: 12,
            response_body: json!({"ok": true}),
            request_headers: json!({}),
            response_headers: json!({}),
        }
    }

    #[test]
    fn run_ids_are_recognisable_and_unique() {
        let runs = PendingRuns::new();
        let (a, _) = begin(&runs);
        let (b, _) = begin(&runs);
        assert!(is_run_id(&a) && is_run_id(&b));
        assert_ne!(a, b);
        assert!(!is_run_id("E0001-AAA"));
    }

    #[test]
    fn a_run_in_flight_cannot_be_saved_yet() {
        let runs = PendingRuns::new();
        let (id, _) = begin(&runs);
        assert_eq!(runs.take_finished(&id).unwrap_err(), TakeError::StillRunning);
        assert_eq!(runs.take_finished("run-nope").unwrap_err(), TakeError::NotFound);
        // still there after the refused take
        assert!(runs.finish(&id, result()).is_some());
    }

    #[test]
    fn finishing_stops_the_countdown_and_makes_it_saveable() {
        let runs = PendingRuns::new();
        let (id, token) = begin(&runs);
        assert!(runs.finish(&id, result()).is_some());
        assert!(token.is_cancelled());
        let run = runs.take_finished(&id).unwrap();
        assert_eq!(run.result.as_ref().unwrap().status_code, 200);
        assert_eq!(run.endpoint_str, "https://x.test/a");
        // taken once: a second save finds nothing
        assert_eq!(runs.take_finished(&id).unwrap_err(), TakeError::NotFound);
    }

    #[test]
    fn a_failed_save_can_put_the_run_back() {
        let runs = PendingRuns::new();
        let (id, _) = begin(&runs);
        runs.finish(&id, result());
        let run = runs.take_finished(&id).unwrap();
        runs.restore(run);
        assert!(runs.take_finished(&id).is_ok());
    }

    #[test]
    fn a_discarded_run_is_gone_and_its_countdown_stopped() {
        let runs = PendingRuns::new();
        let (id, token) = begin(&runs);
        assert!(runs.discard(&id));
        assert!(token.is_cancelled());
        assert!(!runs.discard(&id));
        assert!(runs.finish(&id, result()).is_none());
    }

    #[test]
    fn the_store_is_capped_and_drops_finished_runs_first() {
        let runs = PendingRuns::new();
        let (running, _) = begin(&runs);
        for _ in 0..(MAX_RUNS + 20) {
            let (id, _) = begin(&runs);
            runs.finish(&id, result());
        }
        assert!(runs.len() <= MAX_RUNS);
        // the one still in flight is never evicted
        assert_eq!(runs.take_finished(&running).unwrap_err(), TakeError::StillRunning);
    }

    #[tokio::test]
    async fn the_countdown_times_the_run_out_and_drops_it() {
        let runs = PendingRuns::new();
        let (tx, mut rx) = broadcast::channel(16);
        let token = CancellationToken::new();
        let id = runs.begin(None, "u".into(), "GET".into(), None, json!({}), token.clone());
        spawn_run_timer(
            id.clone(),
            token,
            TimerConfig { limit_ms: 60, tick_interval_ms: 20 },
            tx,
            runs.clone(),
        );
        let mut saw_timeout = false;
        for _ in 0..10 {
            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Ok(ServerEvent::RunTimeout { run_id })) => {
                    assert_eq!(run_id, id);
                    saw_timeout = true;
                    break;
                }
                Ok(Ok(_)) => continue,
                _ => break,
            }
        }
        assert!(saw_timeout);
        assert_eq!(runs.take_finished(&id).unwrap_err(), TakeError::NotFound);
    }

    #[tokio::test]
    async fn a_result_before_the_limit_means_no_timeout_event() {
        let runs = PendingRuns::new();
        let (tx, mut rx) = broadcast::channel(16);
        let token = CancellationToken::new();
        let id = runs.begin(None, "u".into(), "GET".into(), None, json!({}), token.clone());
        spawn_run_timer(
            id.clone(),
            token,
            TimerConfig { limit_ms: 200, tick_interval_ms: 20 },
            tx,
            runs.clone(),
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        runs.finish(&id, result());
        tokio::time::sleep(Duration::from_millis(300)).await;
        while let Ok(evt) = rx.try_recv() {
            assert!(!matches!(evt, ServerEvent::RunTimeout { .. }));
        }
        assert!(runs.take_finished(&id).is_ok());
    }
}
