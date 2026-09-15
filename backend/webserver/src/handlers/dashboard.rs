use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::{dashboard_db, ipc, state::AppState};

/// GET /dashboard/snapshot
/// Returns the most recent dashboard snapshot (endpoint/bookmark/tag counts,
/// main DB size, storage size, file count) plus the current process's
/// start timestamp, which isn't stored per-snapshot since it doesn't change
/// between snapshots.
pub async fn get_dashboard_snapshot(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let conn = state.dashboard_conn.lock().unwrap();
    match dashboard_db::get_latest_snapshot(&conn) {
        Ok(Some(snapshot)) => {
            let mut body = json!(snapshot);
            body["app_started_at"] = json!(state.started_at);
            (StatusCode::OK, Json(body))
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no dashboard snapshot available yet" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e}") })),
        ),
    }
}

/// GET /dashboard/static
/// Returns the hardcoded static data loaded from config.yaml at startup:
/// application limits, stability/commit info, and external links.
pub async fn get_static_data(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!(state.config.as_ref())))
}

/// GET /dashboard/snapshot/history
/// Returns every retained snapshot (rolling 30-day window), oldest first,
/// for trend charts.
pub async fn get_dashboard_snapshot_history(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let conn = state.dashboard_conn.lock().unwrap();
    match dashboard_db::get_snapshot_history(&conn) {
        Ok(snapshots) => (StatusCode::OK, Json(json!(snapshots))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e}") })),
        ),
    }
}

/// POST /dashboard/crud-operations/refresh
/// Ravi's [1] approach — global, one-time processing via a compute child
/// process, triggered only by this route (and app startup), never on a
/// per-transaction basis. Fire-and-forget: result lands later via
/// /internal/callback. This is what the dashboard's Refresh button hits.
pub async fn refresh_crud_operations(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    ipc::spawn_child(
        "compute_crud_operations",
        json!({ "db_path": state.db_path.display().to_string() }),
        3000,
    );

    (
        StatusCode::ACCEPTED,
        Json(json!({ "ok": true, "status": "crud operations refresh queued" })),
    )
}

/// GET /dashboard/crud-operations
/// Returns the latest computed CRUD Operations breakdown (entity type x
/// HTTP method, count + percentage of that row's total), plus when it was
/// last computed. Empty until the first refresh completes.
pub async fn get_crud_operations(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let conn = state.dashboard_conn.lock().unwrap();
    match dashboard_db::get_crud_operations(&conn) {
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no CRUD operations data yet — trigger POST /dashboard/crud-operations/refresh" })),
        ),
        Ok(Some((computed_at, rows))) => {
            // Group by entity_type, then compute each cell's % of that row's total.
            let mut by_entity: HashMap<String, Vec<(String, i64)>> = HashMap::new();
            for row in &rows {
                by_entity
                    .entry(row.entity_type.clone())
                    .or_default()
                    .push((row.method.clone(), row.count));
            }

            let mut entities = serde_json::Map::new();
            for (entity_type, methods) in by_entity {
                let total: i64 = methods.iter().map(|(_, c)| c).sum();
                let mut cells = serde_json::Map::new();
                for (method, count) in methods {
                    let pct = if total > 0 {
                        (count as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };
                    cells.insert(method, json!({ "count": count, "pct": pct }));
                }
                entities.insert(entity_type, json!(cells));
            }

            (
                StatusCode::OK,
                Json(json!({ "computed_at": computed_at, "rows": entities })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e}") })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct CompareQuery {
    pub from: String,
    pub to: String,
}

/// GET /dashboard/compare?from=YYYY-MM-DD&to=YYYY-MM-DD
/// Day-over-day comparison between two daily snapshots (arbitrary dates, not
/// necessarily adjacent — e.g. day 1 vs day 6). 404s per-date if that date
/// has no daily snapshot, distinguishing "outside the 30-day retention
/// window" (a permanent condition for that date) from "no data yet" (today,
/// the future, or the app wasn't running at that day's rollover).
pub async fn compare_daily_snapshots(
    State(state): State<AppState>,
    Query(params): Query<CompareQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let conn = state.dashboard_conn.lock().unwrap();
    let cutoff = (Utc::now().date_naive() - chrono::Duration::days(30)).to_string();

    let lookup = |date: &str| -> Result<dashboard_db::DailySnapshot, (StatusCode, serde_json::Value)> {
        match dashboard_db::get_daily_snapshot(&conn, date) {
            Ok(Some(snap)) => Ok(snap),
            Ok(None) if date < cutoff.as_str() => Err((
                StatusCode::NOT_FOUND,
                json!({ "error": format!("no snapshot for {date} — outside the 30-day retention window") }),
            )),
            Ok(None) => Err((
                StatusCode::NOT_FOUND,
                json!({ "error": format!("no snapshot for {date} yet") }),
            )),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": format!("{e}") }),
            )),
        }
    };

    let from_snap = match lookup(&params.from) {
        Ok(s) => s,
        Err((code, body)) => return (code, Json(body)),
    };
    let to_snap = match lookup(&params.to) {
        Ok(s) => s,
        Err((code, body)) => return (code, Json(body)),
    };

    let diff = json!({
        "endpoint_count": to_snap.endpoint_count - from_snap.endpoint_count,
        "bookmark_count": to_snap.bookmark_count - from_snap.bookmark_count,
        "unique_tag_count": to_snap.unique_tag_count - from_snap.unique_tag_count,
        "total_tag_count": to_snap.total_tag_count - from_snap.total_tag_count,
        "sqlite_size_mb": to_snap.sqlite_size_mb - from_snap.sqlite_size_mb,
        "storage_size_mb": to_snap.storage_size_mb - from_snap.storage_size_mb,
        "file_count": to_snap.file_count - from_snap.file_count,
    });

    (
        StatusCode::OK,
        Json(json!({ "from": from_snap, "to": to_snap, "diff": diff })),
    )
}

/// Called from the /internal/callback handler once the compute_crud_operations
/// child process reports back — writes the result into dashboard.db and
/// returns the computed_at timestamp used, for the caller to broadcast.
pub fn store_crud_operations_result(
    state: &AppState,
    result: &serde_json::Value,
) -> Result<String, String> {
    let rows: Vec<dashboard_db::CrudOperationsRow> = result["rows"]
        .as_array()
        .ok_or("missing 'rows' in compute_crud_operations result")?
        .iter()
        .map(|r| dashboard_db::CrudOperationsRow {
            entity_type: r["entity_type"].as_str().unwrap_or("").to_string(),
            method: r["method"].as_str().unwrap_or("").to_string(),
            count: r["count"].as_i64().unwrap_or(0),
        })
        .collect();

    let computed_at = Utc::now().to_rfc3339();
    let conn = state.dashboard_conn.lock().unwrap();
    dashboard_db::store_crud_operations(&conn, &computed_at, &rows).map_err(|e| e.to_string())?;
    Ok(computed_at)
}

/// GET /purge_audit_report
/// Retrieves the most recent audit report from disk (temp/audit.json)
/// or generates one on-the-fly.
pub async fn get_purge_audit_report(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let report_path = state.storage_root.join("temp").join("audit.json");
    if report_path.exists() {
        if let Ok(file) = std::fs::File::open(&report_path) {
            if let Ok(report) = serde_json::from_reader::<_, serde_json::Value>(file) {
                return (StatusCode::OK, Json(report));
            }
        }
    }

    let eqp_dir = state.storage_root.join("storage").join("globalEQPData");
    match compute::audit_orphanedEIDs::generate_audit_report(
        &state.db_path,
        &eqp_dir,
        &report_path,
    ) {
        Ok(report) => (StatusCode::OK, Json(json!(report))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e}") })),
        ),
    }
}

/// POST /purge_orphaned
/// Executes the purge of orphaned disk folders and DB rows based on the audit report.
pub async fn execute_purge_orphaned(
    State(state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    let report_path = state.storage_root.join("temp").join("audit.json");
    let eqp_dir = state.storage_root.join("storage").join("globalEQPData");

    let report = if report_path.exists() {
        if let Ok(file) = std::fs::File::open(&report_path) {
            serde_json::from_reader::<_, compute::audit_orphanedEIDs::AuditReport>(file).ok()
        } else {
            None
        }
    } else {
        None
    };

    let report = match report {
        Some(r) => r,
        None => {
            match compute::audit_orphanedEIDs::generate_audit_report(
                &state.db_path,
                &eqp_dir,
                &report_path,
            ) {
                Ok(r) => r,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to generate audit report: {e}") })),
                    );
                }
            }
        }
    };

    match compute::audit_orphanedEIDs::execute_purge(
        &state.db_path,
        &eqp_dir,
        &report,
    ) {
        Ok(result) => {
            let _ = std::fs::remove_file(&report_path);
            (StatusCode::OK, Json(json!(result)))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{e}") })),
        ),
    }
}


