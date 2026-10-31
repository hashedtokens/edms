//! Long-running Import/Export work (takeout, import, unzip, move...) runs in
//! an `edms-child` process, so the HTTP request that starts it returns
//! `202 {job_id}` straight away and the result arrives later.
//!
//! The IPC is fire-and-forget: the child POSTs one result to
//! `/internal/callback` and nothing links that result to the request that
//! started it. A job is that link. The handler creates one, hands its id to
//! the child in the payload, the child echoes the id back, and the callback
//! handler finishes the job and broadcasts the result as a `ServerEvent`.
//!
//! The table is in memory and bounded: it exists so a UI that missed the
//! WebSocket event (page reload, dropped connection) can still ask
//! `GET /jobs/:id`, and so a child that died without ever calling back shows
//! up as `stalled` instead of silence. It does not survive a restart.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Jobs kept (finished ones are dropped oldest-first beyond this).
const MAX_JOBS: usize = 200;
/// A job still `running` after this long is reported as `stalled`: the child
/// most likely died without calling back.
const STALL_AFTER_SECS: u64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: String,
    /// What it does: "takeout", "import", "unzip", "format_check", "move".
    pub op: String,
    /// "repoview" / "webview" when the job belongs to one, else null.
    pub view: Option<String>,
    /// The view (or folder) it works on.
    pub name: Option<String>,
    pub status: JobStatus,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<String>,
    /// Latest progress the child reported (`{step, done, total}`).
    pub progress: Option<Value>,
    /// What the webserver needs to finish a job once its child calls back
    /// (an import's plan, for instance). Never shown by the API.
    #[serde(skip)]
    pub context: Option<Value>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Job {
    /// The job as the API shows it: `running` past the stall limit reads as
    /// `stalled`.
    pub fn to_json(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(Value::Null);
        if self.status == JobStatus::Running && now_ms().saturating_sub(self.started_at_ms) > STALL_AFTER_SECS * 1000 {
            v["status"] = json!("stalled");
        }
        v["age_ms"] = json!(self.finished_at_ms.unwrap_or_else(now_ms).saturating_sub(self.started_at_ms));
        v
    }
}

#[derive(Clone, Default)]
pub struct JobTable {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    next: u64,
    jobs: VecDeque<Job>,
}

impl JobTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a running job and returns its id.
    pub fn create(&self, op: &str, view: Option<&str>, name: Option<&str>) -> String {
        let mut inner = self.inner.lock().unwrap();
        inner.next += 1;
        let started = now_ms();
        let id = format!("job-{started}-{}", inner.next);
        inner.jobs.push_back(Job {
            id: id.clone(),
            op: op.to_string(),
            view: view.map(str::to_string),
            name: name.map(str::to_string),
            status: JobStatus::Running,
            started_at_ms: started,
            finished_at_ms: None,
            result: None,
            error: None,
            progress: None,
            context: None,
        });
        // Drop the oldest finished jobs once over the cap; never a running one.
        while inner.jobs.len() > MAX_JOBS {
            match inner.jobs.iter().position(|j| j.status != JobStatus::Running) {
                Some(i) => {
                    inner.jobs.remove(i);
                }
                None => break,
            }
        }
        id
    }

    pub fn get(&self, id: &str) -> Option<Job> {
        self.inner.lock().unwrap().jobs.iter().find(|j| j.id == id).cloned()
    }

    /// Newest first.
    pub fn list(&self) -> Vec<Job> {
        self.inner.lock().unwrap().jobs.iter().rev().cloned().collect()
    }

    pub fn set_context(&self, id: &str, context: Value) {
        if let Some(job) = self.inner.lock().unwrap().jobs.iter_mut().find(|j| j.id == id) {
            job.context = Some(context);
        }
    }

    /// Takes the context out (so it is used once).
    pub fn take_context(&self, id: &str) -> Option<Value> {
        self.inner.lock().unwrap().jobs.iter_mut().find(|j| j.id == id)?.context.take()
    }

    pub fn set_progress(&self, id: &str, progress: Value) -> Option<Job> {
        let mut inner = self.inner.lock().unwrap();
        let job = inner.jobs.iter_mut().find(|j| j.id == id && j.status == JobStatus::Running)?;
        job.progress = Some(progress);
        Some(job.clone())
    }

    /// Marks a running job done. A job that already finished is left alone
    /// (a duplicate callback must not rewrite its result) and `None` is
    /// returned.
    pub fn complete(&self, id: &str, result: Value) -> Option<Job> {
        self.finish(id, JobStatus::Done, Some(result), None)
    }

    pub fn fail(&self, id: &str, error: String) -> Option<Job> {
        self.finish(id, JobStatus::Failed, None, Some(error))
    }

    fn finish(&self, id: &str, status: JobStatus, result: Option<Value>, error: Option<String>) -> Option<Job> {
        let mut inner = self.inner.lock().unwrap();
        let job = inner.jobs.iter_mut().find(|j| j.id == id && j.status == JobStatus::Running)?;
        job.status = status;
        job.finished_at_ms = Some(now_ms());
        job.result = result;
        job.error = error;
        Some(job.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_runs_then_completes_once() {
        let jobs = JobTable::new();
        let id = jobs.create("takeout", Some("repoview"), Some("rv"));
        assert_eq!(jobs.get(&id).unwrap().status, JobStatus::Running);

        let done = jobs.complete(&id, json!({"files": 3})).unwrap();
        assert_eq!(done.status, JobStatus::Done);
        assert_eq!(done.result, Some(json!({"files": 3})));
        assert!(done.finished_at_ms.is_some());

        // a duplicate callback, or a late failure, can't rewrite a finished job
        assert!(jobs.complete(&id, json!({"files": 99})).is_none());
        assert!(jobs.fail(&id, "late".into()).is_none());
        assert_eq!(jobs.get(&id).unwrap().result, Some(json!({"files": 3})));
    }

    #[test]
    fn a_failed_job_keeps_its_error() {
        let jobs = JobTable::new();
        let id = jobs.create("import", Some("webview"), None);
        let failed = jobs.fail(&id, "disk full".into()).unwrap();
        assert_eq!(failed.status, JobStatus::Failed);
        assert_eq!(failed.error.as_deref(), Some("disk full"));
        assert!(failed.result.is_none());
    }

    #[test]
    fn progress_only_applies_to_a_running_job() {
        let jobs = JobTable::new();
        let id = jobs.create("takeout", None, None);
        assert!(jobs.set_progress(&id, json!({"done": 1, "total": 4})).is_some());
        assert_eq!(jobs.get(&id).unwrap().progress, Some(json!({"done": 1, "total": 4})));
        jobs.complete(&id, json!({}));
        assert!(jobs.set_progress(&id, json!({"done": 2})).is_none());
    }

    #[test]
    fn the_context_is_kept_private_and_taken_once() {
        let jobs = JobTable::new();
        let id = jobs.create("import", None, None);
        jobs.set_context(&id, json!({"plan": [1, 2]}));
        assert!(jobs.get(&id).unwrap().to_json().get("context").is_none(), "never in the API");
        assert_eq!(jobs.take_context(&id), Some(json!({"plan": [1, 2]})));
        assert_eq!(jobs.take_context(&id), None);
    }

    #[test]
    fn unknown_ids_are_ignored() {
        let jobs = JobTable::new();
        assert!(jobs.get("nope").is_none());
        assert!(jobs.complete("nope", json!({})).is_none());
        assert!(jobs.fail("nope", "x".into()).is_none());
    }

    #[test]
    fn ids_are_unique_and_the_list_is_newest_first() {
        let jobs = JobTable::new();
        let a = jobs.create("takeout", None, None);
        let b = jobs.create("takeout", None, None);
        assert_ne!(a, b);
        assert_eq!(jobs.list().iter().map(|j| j.id.clone()).collect::<Vec<_>>(), vec![b, a]);
    }

    #[test]
    fn the_table_is_bounded_and_never_drops_a_running_job() {
        let jobs = JobTable::new();
        let running = jobs.create("import", None, None);
        for _ in 0..(MAX_JOBS + 20) {
            let id = jobs.create("takeout", None, None);
            jobs.complete(&id, json!({}));
        }
        assert!(jobs.list().len() <= MAX_JOBS + 1);
        assert_eq!(jobs.get(&running).unwrap().status, JobStatus::Running);
    }

    #[test]
    fn a_job_running_too_long_reads_as_stalled() {
        let jobs = JobTable::new();
        let id = jobs.create("takeout", None, None);
        let mut job = jobs.get(&id).unwrap();
        assert_eq!(job.to_json()["status"], "running");
        job.started_at_ms = now_ms() - (STALL_AFTER_SECS + 5) * 1000;
        assert_eq!(job.to_json()["status"], "stalled");
        // finished jobs never read as stalled
        job.status = JobStatus::Done;
        assert_eq!(job.to_json()["status"], "done");
    }
}
