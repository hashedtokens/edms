
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::events::ServerEvent;

#[derive(Debug, Clone)]
pub struct TimerConfig {
   
    pub limit_ms: u64,
    
    pub tick_interval_ms: u64,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            limit_ms: 30_000,       // 30 seconds
            tick_interval_ms: 500,  
        }
    }
}

#[derive(Clone)]
pub struct TimerHandle {
    pub cancel: CancellationToken,
    started_at: Instant,
}

impl TimerHandle {
    
    pub fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}


pub fn spawn_timer(
    endpoint_id: String,
    request_number: i32,
    config: TimerConfig,
    events_tx: broadcast::Sender<ServerEvent>,
) -> TimerHandle {
    let token = CancellationToken::new();
    let handle = TimerHandle {
        cancel: token.clone(),
        started_at: Instant::now(),
    };

    let eid = endpoint_id.clone();
    let rn = request_number;
    let limit = config.limit_ms;
    let interval = Duration::from_millis(config.tick_interval_ms);

    tokio::spawn(async move {
        let start = Instant::now();

        loop {
            
            tokio::select! {
                _ = tokio::time::sleep(interval) => {}
                _ = token.cancelled() => {
                    // Test finished — emit cancellation event and exit.
                    let elapsed = start.elapsed().as_millis() as u64;
                    let _ = events_tx.send(ServerEvent::TimerCancelled {
                        endpoint_id: eid.clone(),
                        request_number: rn,
                        elapsed_ms: elapsed,
                    });
                    debug!(endpoint = %eid, "timer cancelled after {elapsed}ms");
                    return;
                }
            }

            let elapsed = start.elapsed().as_millis() as u64;
            let remaining = limit.saturating_sub(elapsed);

            // Emit a tick.
            let _ = events_tx.send(ServerEvent::TimerTick {
                endpoint_id: eid.clone(),
                request_number: rn,
                elapsed_ms: elapsed,
                remaining_ms: remaining,
                limit_ms: limit,
            });

           
            if elapsed >= limit {
                let _ = events_tx.send(ServerEvent::TestTimeout {
                    endpoint_id: eid.clone(),
                    request_number: rn,
                });
                debug!(endpoint = %eid, "timer expired after {limit}ms");
                return;
            }
        }
    });

    handle
}