use crate::cgroup_v1::Cgroup;
use crate::child::run_child;
use crate::pipe::{self, PipeRx};
use crate::proc::{clone_proc, wait_child};
use crate::signal;
use crate::{SandboxConfig, SandboxOutput};

use std::ptr;
use std::time::Instant;

use aligned_utils::bytes::AlignedBytes;
use anyhow::{Context, Result};
use nix::sched::CloneFlags;
use nix::unistd::Pid;
use scopeguard::guard;
use tracing::{trace, warn};

#[tracing::instrument(level = "trace", err, skip(config), fields(nonce))]
pub fn run(config: &SandboxConfig) -> Result<SandboxOutput> {
    let nonce: u32 = rand::random();
    tracing::Span::current().record("nonce", &nonce);

    trace!(?config);

    validate(&config)?;

    let cgroup = Cgroup::create(&format!("carapace_{}", nonce))?;

    let (pipe_tx, pipe_rx) = pipe::create().context("failed to create pipe")?;

    let (t0, child_pid) = {
        let clone_cb = || unsafe {
            let pipe_tx = ptr::read(&pipe_tx);
            let pipe_rx = ptr::read(&pipe_rx);
            drop(pipe_rx);

            let result = run_child(&config, &cgroup);

            let _ = pipe_tx.write_error(result.unwrap_err());
            101
        };

        let mut stack = AlignedBytes::new_zeroed(128 * 1024, 16);

        let flags: CloneFlags = CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET;

        // NOTE:
        // When the last process in an IPC namespace exits,
        // all IPC objects in the namespace are automatically destroyed.
        // But it can cause an overhead (about 30ms) of shutting down the last process,
        // which increase the `real_time` number in sandbox output.
        // Is it a kernel bug?
        //
        // REF: https://man7.org/linux/man-pages/man7/ipc_namespaces.7.html

        let t0 = Instant::now();

        let child_pid = unsafe { clone_proc(clone_cb, &mut *stack, flags, libc::SIGCHLD) }
            .context("failed to fork")?;

        (t0, child_pid)
    };

    drop(pipe_tx);
    run_parent(config, child_pid, t0, pipe_rx, cgroup)
}

fn validate(config: &SandboxConfig) -> Result<()> {
    if let Some(prio) = config.priority {
        if !(-20..20).contains(&prio) {
            anyhow::bail!("priority must be in the range -20 to 19: prio = {}", prio);
        }
    }

    for mnt in config.bindmount_rw.iter().chain(config.bindmount_ro.iter()) {
        if !mnt.src.is_absolute() || !mnt.dst.is_absolute() {
            anyhow::bail!(
                "bind mount path must be absolute: src = {}, dst = {}",
                mnt.src.display(),
                mnt.dst.display()
            )
        }
    }

    for mnt in config.mount_proc.iter().chain(&config.mount_tmpfs) {
        if !mnt.is_absolute() {
            anyhow::bail!(
                "special mount path must be absolute: path = {}",
                mnt.display()
            )
        }
    }

    Ok(())
}

fn run_parent(
    config: &SandboxConfig,
    child_pid: Pid,
    t0: Instant,
    pipe_rx: PipeRx,
    cgroup: Cgroup,
) -> Result<SandboxOutput> {
    trace!(?child_pid);

    let killer: Option<_> = if let Some(real_time_limit) = config.real_time_limit {
        let handle = signal::async_kill(child_pid, real_time_limit);
        Some(guard(handle, |h| h.abort()))
    } else {
        None
    };

    trace!("start to receive child result");

    let child_result = pipe_rx
        .read_result()
        .context("failed to read child result")?;

    let child_result_duration = t0.elapsed();
    trace!(?child_result_duration);

    child_result.context("child process failed")?;

    let wait_t0 = Instant::now();
    let (code, signal) = wait_child(child_pid).context("failed to wait4")?;
    let wait_duration = wait_t0.elapsed();
    let real_duration = t0.elapsed();
    drop(killer);

    trace!(?code, ?signal, ?real_duration, ?wait_duration);

    let m = {
        let ret1 = cg_collect(&cgroup).context("failed to collect metrics from cgroup");
        let ret2 = cg_cleanup(cgroup).context("failed to cleanup cgroup");
        ret2.and(ret1)?
    };

    Ok(SandboxOutput {
        code,
        signal,
        real_time: real_duration.as_millis() as u64,
        sys_time: m.sys_time / 1_000_000,   // ns => ms
        user_time: m.user_time / 1_000_000, // ns => ms
        memory: m.memory / 1024,            // bytes => KiB
    })
}

#[derive(Debug)]
struct Metrics {
    sys_time: u64,  // ns
    user_time: u64, // ns
    memory: u64,    // bytes
}

fn cg_collect(cg: &Cgroup) -> Result<Metrics> {
    let sys_time = Cgroup::read_type::<u64>(cg.cpu(), "cpuacct.usage_sys")?;
    let user_time = Cgroup::read_type::<u64>(cg.cpu(), "cpuacct.usage_user")?;
    let memory = Cgroup::read_type::<u64>(cg.memory(), "memory.max_usage_in_bytes")?;

    let metrics = Metrics {
        sys_time,
        user_time,
        memory,
    };

    trace!(?metrics);

    Ok(metrics)
}

fn cg_cleanup(cg: Cgroup) -> Result<()> {
    let content =
        Cgroup::read_string(&cg.cpu(), "cgroup.procs").context("failed to read cgroup procs")?;

    if !content.is_empty() {
        let mut pids = Vec::new();
        for t in content.split('\n') {
            if !t.is_empty() {
                let pid = t.parse::<i32>().unwrap();
                pids.push(Pid::from_raw(pid))
            }
        }
        trace!(?pids);
        signal::killall(&pids);
    }

    if let Err(err) = Cgroup::remove_dir(cg.cpu()) {
        warn!(path = ?cg.cpu(), %err, "failed to remove cgroup dir")
    }
    if let Err(err) = Cgroup::remove_dir(cg.memory()) {
        warn!(path = ?cg.memory(), %err, "failed to remove cgroup dir")
    }
    if let Err(err) = Cgroup::remove_dir(cg.pids()) {
        warn!(path = ?cg.pids(), %err, "failed to remove cgroup dir")
    }

    Ok(())
}
