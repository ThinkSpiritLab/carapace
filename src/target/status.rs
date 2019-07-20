use nix::sys::signal::Signal;

#[derive(Debug)]
pub struct TargetStatus {
    pub code: Option<i32>,
    pub signal: Option<Signal>,
    pub real_time: u64, // in microseconds
    pub user_time: u64, // in microseconds
    pub sys_time: u64,  // in microseconds
    pub memory: u64,    // in kilobytes
}
