use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TargetStatus {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub real_time: u64, // in microseconds
    pub user_time: u64, // in microseconds
    pub sys_time: u64,  // in microseconds
    pub memory: u64,    // in kilobytes
}
