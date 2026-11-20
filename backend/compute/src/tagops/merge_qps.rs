//! The QPs of a merge. Merging an endpoint's QPs into another endpoint is
//! storing each of them there the way a Run does (`QpStore`): the duplicate
//! check, the next request number, the three files, and the request/response
//! rows the app lists a QP from. The files alone are not a QP the app can see.
//!
//! A QP the target endpoint already has (the same request, response and
//! headers) is not copied again, so merging the same collection twice adds
//! nothing the second time.

use std::collections::HashMap;

use edms::core::EdmsCore;

use crate::qp_service;
use crate::qp_store::{NewQp, QpStore, Stored};

/// What copying the QPs of one endpoint did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct QpCopy {
    /// QPs stored under the target endpoint
    pub added: usize,
    /// QPs the target endpoint already had
    pub already_there: usize,
    /// Bytes the target endpoint grew by
    pub bytes: i64,
}

/// What a QP without a headers file (or without a response file) is stored
/// with: nothing was recorded, so nothing is made up.
const NO_HEADERS: &str = r#"{"request_headers":{},"response_headers":{}}"#;
const NO_RESPONSE: &str = "null";

/// The status code and the time of each of the source's QPs, by number.
fn outcomes(core: &EdmsCore, eid: &str) -> HashMap<i32, (Option<i32>, Option<i32>)> {
    core.cproc(
        "SELECT request_number, status_code, response_time_ms FROM response_metadata WHERE endpoint_id = ?",
        &[&eid],
        |r| Ok((r.get::<_, i32>(0)?, r.get::<_, Option<i32>>(1)?, r.get::<_, Option<i32>>(2)?)),
    )
    .map(|rows| rows.into_iter().map(|(n, status, time)| (n, (status, time))).collect())
    .unwrap_or_default()
}

/// Stores every QP of `src_eid` under `dest_eid`. `endpoint_str` and `method`
/// are the destination endpoint's, for the duplicate check.
pub fn copy_qps(
    store: &QpStore,
    src_eid: &str,
    dest_eid: &str,
    endpoint_str: &str,
    method: &str,
) -> Result<QpCopy, String> {
    let src_dir = store.eqp_dir(src_eid);
    let numbers = qp_service::list_existing_qp_numbers(&src_dir, src_eid);
    if numbers.is_empty() {
        return Ok(QpCopy::default());
    }
    let outcome = outcomes(store.core(), src_eid);

    let mut copied = QpCopy::default();
    for n in numbers {
        let read = |kind: &str| std::fs::read_to_string(src_dir.join(format!("{src_eid}-{kind}-{n}.json")));
        let request = read("request").map_err(|e| format!("can't read request {n} of {src_eid}: {e}"))?;
        let response = read("response").unwrap_or_else(|_| NO_RESPONSE.to_string());
        let headers = read("headers").unwrap_or_else(|_| NO_HEADERS.to_string());
        let (status_code, response_time_ms) = outcome.get(&n).copied().unwrap_or((None, None));

        let (stored, bytes) = store.store_qp_deferred(&NewQp {
            endpoint_id: dest_eid,
            endpoint_str,
            method,
            request: &request,
            response: &response,
            headers: &headers,
            status_code,
            response_time_ms,
        })?;
        match stored {
            Stored::New(_) => {
                copied.added += 1;
                copied.bytes += bytes;
            }
            Stored::Duplicate(_) => copied.already_there += 1,
        }
    }
    Ok(copied)
}
