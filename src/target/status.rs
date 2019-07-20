use nix::sys::signal::Signal;

pub struct TargetStatus {
    pub code: Option<i32>,
    pub signal: Option<Signal>,
}
