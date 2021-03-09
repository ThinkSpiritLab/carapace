use crate::cgroup_v1::Cgroup;
use crate::pipe::{self, PipeRx};
use crate::signal;
use crate::utils::{self, libc_call, RawFd};
use crate::{SandboxConfig, SandboxOutput};

use std::convert::Infallible;
use std::ffi::CString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::time::Instant;
use std::{env, io, process, ptr};

use anyhow::{Context, Result};
use nix::fcntl::{self, OFlag};
use nix::sys::stat::Mode;
use nix::unistd::{self, Gid, Pid, Uid};
use rlimit::{Resource, Rlim};
use scopeguard::guard;
use tracing::{trace, warn};

pub fn run(config: &SandboxConfig) -> Result<SandboxOutput> {
    validate(config)?;

    let cgroup = {
        let nonce: u32 = rand::random();
        let cg_name = format!("carapace_{}", nonce);
        Cgroup::create(&cg_name)?
    };

    let (pipe_tx, pipe_rx) = pipe::create().context("failed to create pipe")?;

    let t0 = Instant::now();

    match unsafe { unistd::fork() }.context("failed to fork")? {
        unistd::ForkResult::Parent { child: child_pid } => {
            drop(pipe_tx);
            run_parent(config, child_pid, t0, pipe_rx, cgroup)
        }
        unistd::ForkResult::Child => {
            drop(pipe_rx);
            let result = run_child(&config, cgroup);
            let _ = pipe_tx.write_error(result.unwrap_err());
            process::exit(101);
        }
    }
}

fn validate(config: &SandboxConfig) -> Result<()> {
    if !config.bin.exists() {
        anyhow::bail!(
            "binary file does not exist: path = {}",
            config.bin.display()
        );
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

    let child_result = pipe_rx
        .read_result()
        .context("failed to read child result")?;

    child_result.context("child process failed")?;

    let (status, rusage) = utils::wait4(child_pid).context("failed to wait4")?;
    let real_duration = t0.elapsed();
    let real_time: u64 = real_duration.as_millis() as u64;

    trace!(?status);
    trace!(?rusage);
    trace!(?real_duration);

    drop(killer);

    let code = libc::WEXITSTATUS(status);
    let signal = libc::WTERMSIG(status);

    trace!(?code);
    trace!(?signal);

    let m = {
        let ret1 = cg_collect(&cgroup).context("failed to collect metrics from cgroup");
        let ret2 = cg_cleanup(cgroup).context("failed to cleanup cgroup");
        ret2.and(ret1)?
    };

    Ok(SandboxOutput {
        code,
        signal,
        status,
        real_time,
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

fn run_child(config: &SandboxConfig, cgroup: Cgroup) -> Result<Infallible> {
    let child_pid = unistd::getpid();

    libc_call(|| unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) })?;

    redirect_stdio(config)?;
    set_hard_rlimit(config)?;
    let exec = prepare_execve_args(config)?;

    cg_setup_child(config, &cgroup, child_pid).context("failed to setup cgroup")?;
    cg_reset_metrics(&cgroup).context("failed to reset cgroup metrics")?;

    if let Some(gid) = config.gid.map(Gid::from_raw) {
        unistd::setgroups(&[gid]).context("failed to set groups")?;
        unistd::setgid(gid).context("failed to set gid")?;
    }

    if let Some(uid) = config.uid.map(Uid::from_raw) {
        unistd::setuid(uid).context("failed to set uid")?;
    }

    unsafe { libc::execve(exec.bin, exec.args.as_ptr(), exec.env.as_ptr()) };

    Err(io::Error::last_os_error())
        .with_context(|| format!("failed to execvp: bin = {:?}", config.bin))
}

fn redirect_stdio(config: &SandboxConfig) -> Result<()> {
    fn redirect(file_fd: RawFd, stdio: RawFd) -> nix::Result<()> {
        let ret = unistd::dup2(file_fd, stdio);
        let _ = unistd::close(file_fd);
        ret?;
        Ok(())
    }

    fn get_file_fd(
        path: &Option<PathBuf>,
        fd: Option<RawFd>,
        is_input: bool,
    ) -> nix::Result<Option<RawFd>> {
        if let Some(p) = path {
            if is_input {
                fcntl::open(p, OFlag::O_RDONLY | OFlag::O_CLOEXEC, Mode::empty())
            } else {
                fcntl::open(
                    p,
                    OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_TRUNC | OFlag::O_CLOEXEC,
                    Mode::from_bits_truncate(0o644),
                )
            }
            .map(Some)
        } else if let Some(f) = fd {
            // TODO: check mode
            Ok(Some(f))
        } else {
            Ok(None)
        }
    }

    if let Some(fd) = get_file_fd(&config.stdin, config.stdin_fd, true)? {
        redirect(fd, libc::STDIN_FILENO).context("failed to redirect stdin")?;
    }

    if let Some(fd) = get_file_fd(&config.stdout, config.stdout_fd, false)? {
        redirect(fd, libc::STDOUT_FILENO).context("failed to redirect stdout")?;
    }

    if let Some(fd) = get_file_fd(&config.stderr, config.stderr_fd, false)? {
        redirect(fd, libc::STDERR_FILENO).context("failed to redirect stderr")?;
    }

    Ok(())
}

fn set_hard_rlimit(config: &SandboxConfig) -> Result<()> {
    macro_rules! direct_set {
        ($res:expr, $field:ident) => {
            if let Some($field) = config.$field.map(|r| Rlim::from_raw(r as _)) {
                Resource::AS.set($field, $field)?;
            }
        };
    }

    direct_set!(Resource::CPU, rlimit_cpu);
    direct_set!(Resource::AS, rlimit_as);
    direct_set!(Resource::DATA, rlimit_data);
    direct_set!(Resource::FSIZE, rlimit_fsize);

    Ok(())
}

struct ExecveArgs {
    _cstrings: Vec<CString>,
    bin: *const libc::c_char,
    args: Vec<*const libc::c_char>,
    env: Vec<*const libc::c_char>,
}

fn prepare_execve_args(config: &SandboxConfig) -> Result<ExecveArgs> {
    let mut cstrings = Vec::new();
    let mut args = Vec::new();
    let mut env = Vec::new();

    {
        let c = CString::new(config.bin.as_os_str().as_bytes())?;
        args.push(c.as_ptr());
        cstrings.push(c);
    }
    for a in &config.args {
        let c = CString::new(a.as_bytes())?;
        args.push(c.as_ptr());
        cstrings.push(c);
    }
    args.push(ptr::null());

    for e in &config.env {
        let c = if e.as_bytes().contains(&b'=') {
            CString::new(e.as_bytes())?
        } else if let Some(value) = env::var_os(e) {
            let mut v = Vec::new();
            v.extend_from_slice(e.as_bytes());
            v.push(b'=');
            v.extend(value.into_vec());
            CString::new(v)?
        } else {
            continue;
        };
        env.push(c.as_ptr());
        cstrings.push(c);
    }
    env.push(ptr::null());

    let bin = args[0];

    Ok(ExecveArgs {
        _cstrings: cstrings,
        bin,
        args,
        env,
    })
}

fn cg_setup_child(config: &SandboxConfig, cg: &Cgroup, child_pid: Pid) -> Result<()> {
    Cgroup::add_pid(cg.cpu(), child_pid).context("failed to add pid to cpu cgroup")?;
    Cgroup::add_pid(cg.memory(), child_pid).context("failed to add pid to memory cgroup")?;

    if let Some(memory_limit) = config.cg_limit_memory {
        Cgroup::write_type(cg.memory(), "memory.limit_in_bytes", memory_limit)
            .context("failed to set memory limit")?;
    }

    if let Some(pids_max) = config.cg_limit_max_pids {
        Cgroup::write_type(cg.pids(), "pids.max", pids_max)
            .context("failed to set max pids limit")?;
        Cgroup::add_pid(cg.pids(), child_pid).context("failed to add pid to pids cgroup")?;
    }

    Ok(())
}

fn cg_reset_metrics(cg: &Cgroup) -> Result<()> {
    Cgroup::write_type(cg.cpu(), "cpuacct.usage", 0)?;
    Cgroup::write_type(cg.memory(), "memory.max_usage_in_bytes", 0)?;
    Ok(())
}
