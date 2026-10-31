//! Status codes for the dev API (`/hashedtokens/edms/api/history/` and
//! `/collections/`).
//!
//! A call pushes one QP for one endpoint (endpoint, request, response,
//! headers, plus EDMS data), so a call has one result and one code. The
//! codes are numbered linearly:
//!
//! - `0`: all good
//! - `-1`: unknown, a code not assigned yet
//! - `100`, `101`, `102`, ...: everything else, in the order it was added
//!
//! The same list is kept for every language in `pkg/status_codes.md`, which
//! the Wiki points to. A new code goes at the end and keeps its number; a
//! code is never renumbered or reused. The `matches_the_pkg_list` test fails
//! if this file and `pkg/status_codes.md` drift apart.

use serde::de::Deserializer;
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiStatus {
    Ok,
    Unknown,
    InvalidRequest,
    MissingParam,
    UnknownOp,
    InvalidParam,
    DuplicateQp,
    TestFailed,
    TestTimedOut,
    ComputeUnavailable,
    ComputeStalled,
    InternalError,
}

impl ApiStatus {
    /// Every code, in numeric order after `Ok` and `Unknown`.
    pub const ALL: &'static [ApiStatus] = &[
        ApiStatus::Ok,
        ApiStatus::Unknown,
        ApiStatus::InvalidRequest,
        ApiStatus::MissingParam,
        ApiStatus::UnknownOp,
        ApiStatus::InvalidParam,
        ApiStatus::DuplicateQp,
        ApiStatus::TestFailed,
        ApiStatus::TestTimedOut,
        ApiStatus::ComputeUnavailable,
        ApiStatus::ComputeStalled,
        ApiStatus::InternalError,
    ];

    /// (code, name, message)
    fn row(self) -> (i16, &'static str, &'static str) {
        match self {
            ApiStatus::Ok => (0, "OK", "All good"),
            ApiStatus::Unknown => (-1, "UNKNOWN", "Unknown, or a code not assigned yet"),
            ApiStatus::InvalidRequest => (100, "INVALID_REQUEST", "The request body is not valid JSON or not an object"),
            ApiStatus::MissingParam => (101, "MISSING_PARAM", "A mandatory parameter is missing"),
            ApiStatus::UnknownOp => (102, "UNKNOWN_OP", "The ops value is not one this endpoint supports"),
            ApiStatus::InvalidParam => (103, "INVALID_PARAM", "A parameter has a value that is not allowed"),
            ApiStatus::DuplicateQp => (104, "DUPLICATE_QP", "An identical QP is already stored; it was not added again"),
            ApiStatus::TestFailed => (105, "TEST_FAILED", "The test request failed, so nothing was stored"),
            ApiStatus::TestTimedOut => (106, "TEST_TIMED_OUT", "The test request timed out, so nothing was stored"),
            ApiStatus::ComputeUnavailable => (107, "COMPUTE_UNAVAILABLE", "The compute process could not be started"),
            ApiStatus::ComputeStalled => (108, "COMPUTE_STALLED", "The compute process never reported back"),
            ApiStatus::InternalError => (109, "INTERNAL_ERROR", "The server failed while handling the call"),
        }
    }

    pub fn code(self) -> i16 {
        self.row().0
    }

    /// Stable machine-readable name, e.g. `DUPLICATE_QP`.
    pub fn name(self) -> &'static str {
        self.row().1
    }

    pub fn message(self) -> &'static str {
        self.row().2
    }

    pub fn from_code(code: i16) -> Option<ApiStatus> {
        ApiStatus::ALL.iter().copied().find(|s| s.code() == code)
    }

    /// The HTTP status a route answers with when it must reply directly
    /// instead of `202`: a bad request, or a failure before any work started.
    /// `None` for codes that only ever arrive in the WebSocket result, since
    /// by then the HTTP call was already answered.
    pub fn http_status(self) -> Option<u16> {
        match self {
            ApiStatus::InvalidRequest
            | ApiStatus::MissingParam
            | ApiStatus::UnknownOp
            | ApiStatus::InvalidParam => Some(400),
            ApiStatus::InternalError => Some(500),
            ApiStatus::ComputeUnavailable => Some(503),
            _ => None,
        }
    }
}

/// On the wire a status is `{code, name, message}`. Reading accepts that or
/// just the number, so a compute op can return `104` and be understood.
impl Serialize for ApiStatus {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut st = serializer.serialize_struct("ApiStatus", 3)?;
        st.serialize_field("code", &self.code())?;
        st.serialize_field("name", self.name())?;
        st.serialize_field("message", self.message())?;
        st.end()
    }
}

impl<'de> Deserialize<'de> for ApiStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Number(i16),
            Object { code: i16 },
        }
        let code = match Wire::deserialize(deserializer)? {
            Wire::Number(c) | Wire::Object { code: c } => c,
        };
        ApiStatus::from_code(code)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown API status code {code}")))
    }
}

/// The result of a call, as delivered over the WebSocket. Only the fields
/// that apply are present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiResult {
    pub status: ApiStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_id: Option<String>,
    /// The QP number it was stored as.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_number: Option<i32>,
    /// The QP it duplicates (`DuplicateQp`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplicate_of: Option<i32>,
    /// Whether the call created the endpoint (false: it already existed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_created: Option<bool>,
    /// Whether the call created the collection (collections ops only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_created: Option<bool>,
    /// Extra text for the person reading the log (what was wrong, the error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ApiResult {
    pub fn new(status: ApiStatus) -> Self {
        ApiResult {
            status,
            endpoint_id: None,
            request_number: None,
            duplicate_of: None,
            endpoint_created: None,
            collection_created: None,
            detail: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn codes_are_linear() {
        // 0 and -1 are fixed; everything else counts up from 100 in order.
        assert_eq!(ApiStatus::Ok.code(), 0);
        assert_eq!(ApiStatus::Unknown.code(), -1);
        let rest: Vec<i16> = ApiStatus::ALL.iter().skip(2).map(|s| s.code()).collect();
        let expected: Vec<i16> = (100..100 + rest.len() as i16).collect();
        assert_eq!(rest, expected, "codes must be 100, 101, 102, ... with no gaps");
    }

    #[test]
    fn names_are_unique_and_messages_filled() {
        let names: HashSet<&str> = ApiStatus::ALL.iter().map(|s| s.name()).collect();
        assert_eq!(names.len(), ApiStatus::ALL.len(), "a name appears twice");
        for s in ApiStatus::ALL {
            assert!(!s.message().is_empty());
        }
        assert_eq!(ApiStatus::ALL.len(), 12, "update the count when adding a code");
    }

    #[test]
    fn every_code_round_trips() {
        for s in ApiStatus::ALL {
            assert_eq!(ApiStatus::from_code(s.code()), Some(*s));
        }
        assert_eq!(ApiStatus::from_code(9999), None);
    }

    #[test]
    fn only_codes_a_route_can_answer_with_have_an_http_status() {
        assert_eq!(ApiStatus::UnknownOp.http_status(), Some(400));
        assert_eq!(ApiStatus::InvalidParam.http_status(), Some(400));
        assert_eq!(ApiStatus::ComputeUnavailable.http_status(), Some(503));
        assert_eq!(ApiStatus::InternalError.http_status(), Some(500));
        assert_eq!(ApiStatus::Ok.http_status(), None);
        assert_eq!(ApiStatus::DuplicateQp.http_status(), None);
        assert_eq!(ApiStatus::ComputeStalled.http_status(), None);
    }

    #[test]
    fn a_status_serializes_with_code_name_and_message() {
        let v = serde_json::to_value(ApiStatus::DuplicateQp).unwrap();
        assert_eq!(v["code"], 104);
        assert_eq!(v["name"], "DUPLICATE_QP");
        assert!(v["message"].as_str().unwrap().contains("identical"));
        let unknown = serde_json::to_value(ApiStatus::Unknown).unwrap();
        assert_eq!(unknown["code"], -1);
    }

    #[test]
    fn a_status_reads_from_a_number_or_an_object() {
        let from_number: ApiStatus = serde_json::from_value(json!(0)).unwrap();
        assert_eq!(from_number, ApiStatus::Ok);
        let from_negative: ApiStatus = serde_json::from_value(json!(-1)).unwrap();
        assert_eq!(from_negative, ApiStatus::Unknown);
        let from_object: ApiStatus = serde_json::from_value(json!({"code": 104})).unwrap();
        assert_eq!(from_object, ApiStatus::DuplicateQp);
        assert!(serde_json::from_value::<ApiStatus>(json!(1)).is_err());
    }

    #[test]
    fn a_result_has_the_shape_clients_read() {
        let mut stored = ApiResult::new(ApiStatus::Ok);
        stored.endpoint_id = Some("EID-1".to_string());
        stored.request_number = Some(3);
        stored.endpoint_created = Some(false);
        stored.collection_created = Some(true);
        let v = serde_json::to_value(&stored).unwrap();
        assert_eq!(v["status"]["code"], 0);
        assert_eq!(v["request_number"], 3);
        assert_eq!(v["collection_created"], true);
        assert!(v.get("duplicate_of").is_none());
        let back: ApiResult = serde_json::from_value(v).unwrap();
        assert_eq!(back, stored);

        let mut dup = ApiResult::new(ApiStatus::DuplicateQp);
        dup.duplicate_of = Some(1);
        assert_eq!(serde_json::to_value(&dup).unwrap()["duplicate_of"], 1);
    }

    /// `pkg/status_codes.md` is the list every language reads. This keeps it
    /// and the enum from drifting apart. Skipped when the file isn't there
    /// (a build that only has `backend/`).
    #[test]
    fn matches_the_pkg_list() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../pkg/status_codes.md");
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let mut listed: Vec<(i16, String)> = Vec::new();
        for line in text.lines() {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            if cells.len() < 4 {
                continue;
            }
            if let Ok(code) = cells[1].parse::<i16>() {
                listed.push((code, cells[2].trim_matches('`').to_string()));
            }
        }
        let in_code: Vec<(i16, String)> =
            ApiStatus::ALL.iter().map(|s| (s.code(), s.name().to_string())).collect();
        assert_eq!(listed, in_code, "pkg/status_codes.md and api_status.rs differ");
    }
}
