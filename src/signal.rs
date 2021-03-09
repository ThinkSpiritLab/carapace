use std::time::Duration;

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use tokio::task::{self, JoinHandle};
use tokio::time;
use tracing::trace;

pub fn async_kill(child_pid: Pid, timeout_ms: u64) -> JoinHandle<()> {
    task::spawn(async move {
        time::sleep(Duration::from_millis(timeout_ms)).await;
        let _ = send_signal(child_pid, Signal::SIGKILL);
    })
}

pub fn send_signal(pid: Pid, signal: Signal) -> nix::Result<()> {
    let result = signal::kill(pid, signal);
    trace!(
        "kill pid = {}, signal = {}, result = {:?}",
        pid,
        signal,
        result
    );
    result
}

pub fn killall(pids: &[Pid]) {
    for &pid in pids {
        let _ = send_signal(pid, Signal::SIGSTOP);
    }

    for &pid in pids {
        let _ = send_signal(pid, Signal::SIGKILL);
    }
}
