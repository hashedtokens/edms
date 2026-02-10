
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

use crate::{
    db,
    events::ServerEvent,
    state::AppState,
    timer::{self, TimerConfig},
};

use edms::{error::EdmsError, file_io};


pub async fn ws_run(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_ws_run(socket, state).await;
    })
}

#[derive(Debug, Deserialize)]
struct RunMessage {
    endpoint_id: String,
    method: String,
    #[serde(default)]
    body: Value,

    #[serde(default)]
    timeout_ms: u64,

    
    #[serde(default)]
    tick_interval_ms: u64,
}

impl RunMessage {
    fn timer_config(&self) -> TimerConfig {
        TimerConfig {
            limit_ms: if self.timeout_ms > 0 { self.timeout_ms } else { 30_000 },
            tick_interval_ms: if self.tick_interval_ms > 0 {
                self.tick_interval_ms
            } else {
                500
            },
        }
    }
}

// ── WS handler ───────────────────────────────────────────────────────

async fn handle_ws_run(mut socket: WebSocket, state: AppState) {
    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t) => t,
            _ => continue,
        };

        match serde_json::from_str::<RunMessage>(&text) {
            Ok(run_msg) => {
                let st = state.clone();
              
                tokio::spawn(async move {
                    if let Err(e) = run_test_impl(
                        &st,
                        &run_msg.endpoint_id,
                        &run_msg.method,
                        run_msg.body.clone(),
                        run_msg.timer_config(),
                    )
                    .await
                    {
                        let _ = st.events_tx.send(ServerEvent::Error {
                            message: format!("{e:?}"),
                        });
                    }
                });
            }
            Err(e) => {
                let resp = json!({"type":"error","message": format!("bad message: {e}")});
                let _ = socket.send(Message::Text(resp.to_string())).await;
            }
        }
    }
}

// ── Core test logic with integrated timer ────────────────────────────

async fn run_test_impl(
    state: &AppState,
    endpoint_id: &str,
    method: &str,
    request_json: Value,
    timer_cfg: TimerConfig,
) -> Result<(), EdmsError> {
    // 1) Load endpoint URL from DB
    let endpoint = tokio::task::spawn_blocking({
        let st = state.clone();
        let id = endpoint_id.to_string();
        move || db::get_endpoint(&st.core, &st.queries, &id)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?
    .ok_or(EdmsError::UnknownError)?;

    // 2) Next request number
    let request_number = tokio::task::spawn_blocking({
        let st = state.clone();
        let id = endpoint_id.to_string();
        move || db::get_next_request_number(&st.core, &st.queries, &id)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    // 3) Write request file
    let request_file = tokio::task::spawn_blocking({
        let eid = endpoint_id.to_string();
        let body = request_json.clone();
        move || file_io::write_request_json(&eid, request_number, &body)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    let method_upper = method.to_uppercase();

    tokio::task::spawn_blocking({
        let st = state.clone();
        let eid = endpoint_id.to_string();
        let rf = request_file.clone();
        let m = method_upper.clone();
        move || db::insert_request_metadata(&st.core, &st.queries, &eid, request_number, &rf, &m)
    })
    .await
    .map_err(|_| EdmsError::UnknownError)?
    .map_err(|_| EdmsError::UnknownError)?;

    // 4) Emit TestStarted
    let _ = state.events_tx.send(ServerEvent::TestStarted {
        endpoint_id: endpoint_id.to_string(),
        request_number,
    });

    
    // ═══════════════════════════════════════════════════════════════════
    let timer_handle = timer::spawn_timer(
        endpoint_id.to_string(),
        request_number,
        timer_cfg.clone(),
        state.events_tx.clone(),
    );


    let url = endpoint.endpoint_str.clone();
    let client = state.http_client.clone();
    let timeout_dur = Duration::from_millis(timer_cfg.limit_ms);

    let http_result = tokio::time::timeout(timeout_dur, async {
        let start = Instant::now();

        let resp = match method_upper.as_str() {
            "GET" => client.get(&url).send().await,
            "POST" => client.post(&url).json(&request_json).send().await,
            "PUT" => client.put(&url).json(&request_json).send().await,
            "DELETE" => client.delete(&url).json(&request_json).send().await,
            "PATCH" => client.patch(&url).json(&request_json).send().await,
            "HEAD" => client.head(&url).send().await,
            _ => client.post(&url).json(&request_json).send().await,
        };

        let elapsed_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;

        match resp {
            Ok(r) => {
                let status = r.status().as_u16() as i32;
                let text = r.text().await.unwrap_or_default();
                let body: Value =
                    serde_json::from_str(&text).unwrap_or(json!({ "raw": text }));
                Ok((status, body, elapsed_ms))
            }
            Err(e) => Err(e),
        }
    })
    .await;

    
    timer_handle.cancel();

    // 8) Process result
    match http_result {
        // Completed within the timeout
        Ok(Ok((status_code, response_value, elapsed_ms))) => {
            let response_file = tokio::task::spawn_blocking({
                let eid = endpoint_id.to_string();
                let rv = response_value.clone();
                move || file_io::write_response_json(&eid, request_number, &rv)
            })
            .await
            .map_err(|_| EdmsError::UnknownError)?
            .map_err(|_| EdmsError::UnknownError)?;

            tokio::task::spawn_blocking({
                let st = state.clone();
                let eid = endpoint_id.to_string();
                let rf = response_file.clone();
                move || {
                    db::update_response_metadata(
                        &st.core,
                        &st.queries,
                        &eid,
                        request_number,
                        status_code,
                        elapsed_ms,
                        &rf,
                    )
                }
            })
            .await
            .map_err(|_| EdmsError::UnknownError)?
            .map_err(|_| EdmsError::UnknownError)?;

            let _ = state.events_tx.send(ServerEvent::TestFinished {
                endpoint_id: endpoint_id.to_string(),
                request_number,
                status_code,
                response_time_ms: elapsed_ms,
                response_file,
            });
        }

       
        Ok(Err(e)) => {
            let _ = state.events_tx.send(ServerEvent::Error {
                message: format!("HTTP error for {endpoint_id}: {e}"),
            });
        }

       
        Err(_elapsed) => {
            let _ = state.events_tx.send(ServerEvent::Error {
                message: format!(
                    "{endpoint_id} request #{request_number} timed out after {}ms",
                    timer_cfg.limit_ms
                ),
            });
        }
    }

    Ok(())
}