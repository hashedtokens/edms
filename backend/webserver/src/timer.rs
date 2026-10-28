
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
