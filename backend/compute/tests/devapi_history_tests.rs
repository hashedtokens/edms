// Dev API /api/history/ tests

use compute::devapi::history::{history_ops_inner, parse_history_request};
use compute::qp_store::{NewQp, QpStore, Stored};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

// temp DB (with schema) + storage root, passed the way the webserver will
struct Env {
    _dir: tempfile::TempDir,
    db: PathBuf,
    root: PathBuf,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("edms.db");
    let core = edms::EdmsCore::new(db.to_str().unwrap());
    core.connect().unwrap();
    edms::schema::initialize_schema_from_core(&core).unwrap();
    let root = dir.path().join("root");
    Env { db, root, _dir: dir }
}

impl Env {
    async fn run(&self, mut payload: Value) -> Value {
        payload["db_path"] = json!(self.db.to_str().unwrap());
        payload["storage_root"] = json!(self.root.to_str().unwrap());
        run(payload).await
    }

    fn count(&self, table: &str) -> i64 {
        let conn = Connection::open(&self.db).unwrap();
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }

    fn file(&self, eid: &str, kind: &str, n: i64) -> PathBuf {
        self.root.join("storage/globalEQPData").join(eid).join(format!("{eid}-{kind}-{n}.json"))
    }
}

async fn run(payload: Value) -> Value {
    let out = history_ops_inner(payload).await.expect("compute never errors at the IPC level");
    serde_json::from_str(&out).expect("result is JSON")
}

fn code(r: &Value) -> i64 {
    r["status"]["code"].as_i64().unwrap()
}

// local target: /echo (201), /missing (404), /slow (2s)
async fn start_target() -> u16 {
    use warp::Filter;
    let port = portpicker::pick_unused_port().expect("No free port");

    let echo = warp::path("echo")
        .and(warp::post())
        .and(warp::header::optional::<String>("x-dev"))
        .and(warp::body::json())
        .map(|dev: Option<String>, body: Value| {
            let reply = warp::reply::json(&json!({ "got": body, "x_dev": dev }));
            let reply = warp::reply::with_header(reply, "x-target", "yes");
            warp::reply::with_status(reply, warp::http::StatusCode::CREATED)
        });
    let missing = warp::path("missing")
        .map(|| warp::reply::with_status("nope", warp::http::StatusCode::NOT_FOUND));
    let slow = warp::path("slow").and_then(|| async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok::<_, std::convert::Infallible>("late")
    });

    tokio::spawn(async move {
        warp::serve(echo.or(missing).or(slow)).run(([127, 0, 0, 1], port)).await;
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    port
}

fn echo(port: u16, body: Value) -> Value {
    json!({
        "ops": "test",
        "endpoint_str": format!("http://127.0.0.1:{port}/echo"),
        "method": "post",
        "headers": { "x-dev": "hemang" },
        "body": body,
    })
}

// mandatory / optional params

#[tokio::test]
async fn mandatory_params_are_checked_first() {
    let url = "http://127.0.0.1:1/x";
    for (payload, missing) in [
        (json!({ "endpoint_str": url, "method": "GET" }), "ops"),
        (json!({ "ops": "test", "method": "GET" }), "endpoint_str"),
        (json!({ "ops": "test", "endpoint_str": url }), "method"),
        (json!({ "ops": "test", "endpoint_str": "", "method": "GET" }), "endpoint_str"),
    ] {
        let r = run(payload).await;
        assert_eq!(code(&r), 101, "{r}");
        assert_eq!(r["status"]["name"], "MISSING_PARAM");
        assert!(r["detail"].as_str().unwrap().contains(missing));
    }
}

#[tokio::test]
async fn non_object_and_unknown_op_are_rejected() {
    assert_eq!(code(&run(json!("hi")).await), 100);
    let r = run(json!({ "ops": "insert", "endpoint_str": "http://127.0.0.1:1/x", "method": "GET" })).await;
    assert_eq!(code(&r), 102);
}

#[tokio::test]
async fn bad_values_are_invalid_param() {
    let base = json!({ "ops": "test", "endpoint_str": "http://127.0.0.1:1/x", "method": "GET" });
    let bad = [
        ("endpoint_str", json!("not a url")),
        ("endpoint_str", json!("ftp://example.com/f")),
        ("method", json!("TRACE")),
        ("headers", json!({ "a": 1 })),
        ("timeout_ms", json!("soon")),
        ("annotation", json!(5)),
    ];
    for (key, value) in bad {
        let mut p = base.clone();
        p[key] = value;
        let r = run(p).await;
        assert_eq!(code(&r), 103, "{key}: {r}");
    }
}

#[test]
fn optional_params_default_and_extra_keys_are_allowed() {
    let req = parse_history_request(&json!({
        "ops": "test",
        "endpoint_str": "http://localhost:9/a",
        "method": "post",
        "job_id": "j",
        "endpoint_id": "EID-1"
    }))
    .unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.timeout_ms, 25_000);
    assert!(req.headers.is_empty());
    assert_eq!(req.body, json!({}));
}

#[test]
fn every_method_the_webserver_allows_is_accepted() {
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
        let p = json!({ "ops": "test", "endpoint_str": "http://localhost:9/a", "method": method });
        assert_eq!(parse_history_request(&p).unwrap().method, method);
    }
}

#[test]
fn timeout_stays_under_the_webserver_stall() {
    let parse = |ms: u64| {
        parse_history_request(&json!({
            "ops": "test",
            "endpoint_str": "http://localhost:9/a",
            "method": "GET",
            "timeout_ms": ms
        }))
        .unwrap()
        .timeout_ms
    };
    assert_eq!(parse(200), 200);
    assert_eq!(parse(60_000), 25_000);
}

#[tokio::test]
async fn job_id_is_sent_back_even_on_failure() {
    let r = run(json!({ "job_id": "abc-123" })).await;
    assert_eq!(r["job_id"], "abc-123");
    assert_eq!(code(&r), 101);
}

// ops = test: store like Test View

#[tokio::test]
async fn test_stores_endpoint_qp_files_and_history() {
    let (e, port) = (env(), start_target().await);
    let mut p = echo(port, json!({ "n": 1 }));
    p["annotation"] = json!("from backend");
    p["job_id"] = json!("t1");
    let r = e.run(p).await;

    assert_eq!(code(&r), 0, "{r}");
    assert_eq!(r["endpoint_created"], true);
    assert_eq!(r["target_status"], 201);
    assert_eq!(r["request_number"], 1);
    assert_eq!(r["job_id"], "t1");
    assert_eq!(r["view_type"], "History");
    assert!(r.get("duplicate_of").is_none());

    let eid = r["endpoint_id"].as_str().unwrap();
    for kind in ["request", "response", "headers"] {
        assert!(e.file(eid, kind, 1).exists(), "{kind} file");
    }
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(e.file(eid, "response", 1)).unwrap()).unwrap();
    assert_eq!(saved, json!({ "got": { "n": 1 }, "x_dev": "hemang" }));

    assert_eq!(e.count("endpoints"), 1);
    assert_eq!(e.count("request_metadata"), 1);
    assert_eq!(e.count("response_metadata"), 1);
    assert_eq!(e.count("history"), 1);
    let conn = Connection::open(&e.db).unwrap();
    let (method, annotation): (String, String) = conn
        .query_row("SELECT method, annotation FROM endpoints", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((method.as_str(), annotation.as_str()), ("POST", "from backend"));
}

#[tokio::test]
async fn same_qp_twice_is_blocked_new_body_is_stored() {
    let (e, port) = (env(), start_target().await);
    let first = e.run(echo(port, json!({ "n": 1 }))).await;
    assert_eq!(code(&first), 0, "{first}");

    let again = e.run(echo(port, json!({ "n": 1 }))).await;
    assert_eq!(code(&again), 104, "{again}");
    assert_eq!(again["status"]["name"], "DUPLICATE_QP");
    assert_eq!(again["endpoint_created"], false);
    assert_eq!(again["duplicate_of"], 1);
    assert_eq!(again["target_status"], 201);
    assert!(again.get("view_type").is_none(), "nothing new to refresh");
    assert_eq!(again["endpoint_id"], first["endpoint_id"]);

    let other = e.run(echo(port, json!({ "n": 2 }))).await;
    assert_eq!(code(&other), 0, "{other}");
    assert_eq!(other["endpoint_created"], false);
    assert_eq!(other["request_number"], 2);
    assert_eq!(other["endpoint_id"], first["endpoint_id"]);

    assert_eq!(e.count("endpoints"), 1);
    assert_eq!(e.count("request_metadata"), 2);
    assert_eq!(e.count("history"), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn parallel_calls_take_different_numbers_on_one_endpoint() {
    let (e, port) = (env(), start_target().await);
    let calls: Vec<_> = (0..5)
        .map(|n| {
            let mut p = echo(port, json!({ "n": n }));
            p["db_path"] = json!(e.db.to_str().unwrap());
            p["storage_root"] = json!(e.root.to_str().unwrap());
            tokio::spawn(run(p))
        })
        .collect();

    let mut numbers = Vec::new();
    for call in calls {
        let r = call.await.unwrap();
        assert_eq!(code(&r), 0, "{r}");
        numbers.push(r["request_number"].as_i64().unwrap());
    }
    numbers.sort();
    assert_eq!(numbers, vec![1, 2, 3, 4, 5]);
    assert_eq!(e.count("endpoints"), 1);
    assert_eq!(e.count("history"), 5);
}

#[tokio::test]
async fn an_existing_endpoint_id_is_used_as_given() {
    let (e, port) = (env(), start_target().await);
    let first = e.run(echo(port, json!({ "n": 1 }))).await;

    // same id, different URL: the id wins
    let r = e
        .run(json!({
            "ops": "test",
            "endpoint_str": format!("http://127.0.0.1:{port}/missing"),
            "method": "GET",
            "endpoint_id": first["endpoint_id"]
        }))
        .await;
    assert_eq!(code(&r), 0, "{r}");
    assert_eq!(r["endpoint_id"], first["endpoint_id"]);
    assert_eq!(r["endpoint_created"], false);
    assert_eq!(e.count("endpoints"), 1);
}

#[tokio::test]
async fn history_row_has_the_base_url() {
    let (e, port) = (env(), start_target().await);
    e.run(echo(port, json!({ "n": 1 }))).await;
    let conn = Connection::open(&e.db).unwrap();
    let base: Option<String> = conn.query_row("SELECT base_url FROM history", [], |r| r.get(0)).unwrap();
    assert!(base.is_some());
}

#[test]
fn discard_only_removes_an_endpoint_with_no_qps() {
    let e = env();
    let db = QpStore::open(e.db.to_str().unwrap(), &e.root).unwrap();
    let url = "http://localhost:9/a";

    let (empty, _) = db.find_or_create_endpoint(None, url, "GET", None).unwrap();
    db.discard_endpoint(&empty).unwrap();
    assert_eq!(e.count("endpoints"), 0);

    // another call reused the endpoint and stored a QP before the discard
    let (eid, _) = db.find_or_create_endpoint(None, url, "GET", None).unwrap();
    let stored = db
        .store_qp(&NewQp {
            endpoint_id: &eid,
            endpoint_str: url,
            method: "GET",
            request: "null",
            response: "{}",
            headers: "{}",
            status_code: Some(200),
            response_time_ms: Some(1),
        })
        .unwrap();
    assert_eq!(stored, Stored::New(1));
    db.record_history(&eid, "test", Some("200 in 1ms")).unwrap();

    db.discard_endpoint(&eid).unwrap();
    assert_eq!(e.count("endpoints"), 1);
    assert_eq!(e.count("request_metadata"), 1);
    assert_eq!(e.count("response_metadata"), 1);
    assert_eq!(e.count("history"), 1);
    assert!(e.file(&eid, "request", 1).exists());
}

#[tokio::test]
async fn non_2xx_is_stored_like_test_view() {
    let (e, port) = (env(), start_target().await);
    let r = e
        .run(json!({ "ops": "test", "endpoint_str": format!("http://127.0.0.1:{port}/missing"), "method": "GET" }))
        .await;
    assert_eq!(code(&r), 0, "{r}");
    assert_eq!(r["target_status"], 404);
    assert_eq!(r["request_number"], 1);
    let conn = Connection::open(&e.db).unwrap();
    let status: i64 = conn.query_row("SELECT status_code FROM response_metadata", [], |r| r.get(0)).unwrap();
    assert_eq!(status, 404);
}

#[tokio::test]
async fn timeout_stores_nothing() {
    let (e, port) = (env(), start_target().await);
    let r = e
        .run(json!({
            "ops": "test",
            "endpoint_str": format!("http://127.0.0.1:{port}/slow"),
            "method": "GET",
            "timeout_ms": 200,
            "job_id": "slow-1"
        }))
        .await;
    assert_eq!(code(&r), 106, "{r}");
    assert_eq!(r["status"]["name"], "TEST_TIMED_OUT");
    assert!(r.get("target_status").is_none());
    assert_eq!(r["job_id"], "slow-1");
    assert_eq!(e.count("endpoints"), 0);
    assert_eq!(e.count("history"), 0);
}

#[tokio::test]
async fn unreachable_target_stores_nothing() {
    let e = env();
    let port = portpicker::pick_unused_port().unwrap();
    let r = e
        .run(json!({ "ops": "test", "endpoint_str": format!("http://127.0.0.1:{port}/x"), "method": "GET" }))
        .await;
    assert_eq!(code(&r), 105, "{r}");
    assert_eq!(r["status"]["name"], "TEST_FAILED");
    assert!(r.get("view_type").is_none());
    assert!(r.get("target_status").is_none());
    assert_eq!(e.count("endpoints"), 0);
}
