use crate::cgroup_v1::Cgroup;
use crate::mount::{bind_mount, make_root_private, mount_proc, mount_tmpfs};
use crate::pipe::{self, PipeRx};
use crate::proc::{clone_proc, wait_child};
use crate::signal;
use crate::utils::RawFd;
use crate::{SandboxConfig, SandboxOutput};

use std::borrow::Cow;
use std::convert::{Infallible, TryInto};
use std::ffi::{CString, OsString};
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, fs, io, ptr};

use aligned_utils::bytes::AlignedBytes;
use anyhow::{Context, Result};
use nix::fcntl::{self, OFlag};
use nix::sched::CloneFlags;
use nix::sys::stat::Mode;
use nix::unistd::{self, AccessFlags, Gid, Pid, Uid};
use path_absolutize::Absolutize;
use rlimit::{Resource, Rlim};
use scopeguard::guard;
use tracing::{trace, warn};

pub fn run(config: &SandboxConfig) -> Result<SandboxOutput> {
    validate(&config)?;

    let cgroup = {
        let nonce: u32 = rand::random();
        let cg_name = format!("carapace_{}", nonce);
        Cgroup::create(&cg_name)?
    };

    let (pipe_tx, pipe_rx) = pipe::create().context("failed to create pipe")?;

    let (t0, child_pid) = {
        let clone_cb = || unsafe {
            let pipe_tx = ptr::read(&pipe_tx);
            let pipe_rx = ptr::read(&pipe_rx);
            drop(pipe_rx);

            let result = tracing::Span::none().in_scope(|| run_child(&config, &cgroup));

            let _ = pipe_tx.write_error(result.unwrap_err());
            101
        };

        let mut stack = AlignedBytes::new_zeroed(128 * 1024, 16);

        let flags: CloneFlags = CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET;

        let t0 = Instant::now();

        let child_pid = unsafe { clone_proc(clone_cb, &mut *stack, flags, libc::SIGCHLD) }
            .context("failed to fork")?;

        (t0, child_pid)
    };

    drop(pipe_tx);
    run_parent(config, child_pid, t0, pipe_rx, cgroup)
}

fn validate(config: &SandboxConfig) -> Result<()> {
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

fn run_child(config: &SandboxConfig, cgroup: &Cgroup) -> Result<Infallible> {
    do_mount(&config)?;

    let exec = prepare_execve_args(config)?;

    cg_setup_child(config, cgroup).context("failed to setup cgroup")?;

    let reset: _ = cg_prepare_reset_metrics(cgroup).context("failed to prepare cgroup metrics")?;

    if let Some(ref new_root) = config.chroot {
        unistd::chroot(new_root).context("failed to chroot")?;
        unistd::chdir("/")?;
    }

    set_hard_rlimit(config)?;

    redirect_stdio(config)?;

    unistd::access(&config.bin, AccessFlags::F_OK)
        .with_context(|| format!("failed to access file: path = {}", config.bin.display()))?;

    reset().context("failed to reset cgroup metrics")?;

    set_id(config)?;

    unsafe { libc::execve(exec.bin, exec.args.as_ptr(), exec.env.as_ptr()) };

    Err(io::Error::last_os_error())
        .with_context(|| format!("failed to execve: bin = {:?}", config.bin))
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

fn do_mount(config: &SandboxConfig) -> Result<()> {
    make_root_private()?;

    let root = if let Some(ref chroot) = config.chroot {
        chroot.absolutize()?
    } else {
        Cow::Borrowed("/".as_ref())
    };

    let get_real_dst = |dst: &Path| -> Result<OsString> {
        let dst = dst.absolutize_virtually("/")?;
        let mut real_dst: OsString = root.as_os_str().into();
        real_dst.push(dst.as_os_str());
        Ok(real_dst)
    };

    let rw_mnts: _ = config.bindmount_rw.iter().map(|m| (m, false));
    let ro_mnts: _ = config.bindmount_ro.iter().map(|m| (m, true));

    for (mnt, readonly) in rw_mnts.chain(ro_mnts) {
        let real_dst = get_real_dst(&mnt.dst)?;
        let src: &Path = &mnt.src;
        let dst: &Path = real_dst.as_ref();
        let on_err = || {
            format!(
                "failed to do bind mount: src = {}, dst = {}, readonly = {}",
                src.display(),
                dst.display(),
                readonly
            )
        };
        bind_mount(src, dst, true, readonly).with_context(on_err)?;
    }

    if let Some(ref mnt) = config.mount_proc {
        let real_dst = get_real_dst(mnt)?;
        let dst: &Path = real_dst.as_ref();
        mount_proc(dst)
            .with_context(|| format!("failed to mount proc: dst = {}", dst.display()))?;
    }

    if let Some(ref mnt) = config.mount_tmpfs {
        let real_dst = get_real_dst(mnt)?;
        let dst: &Path = real_dst.as_ref();
        mount_tmpfs(dst)
            .with_context(|| format!("failed to mount tmpfs: dst = {}", dst.display()))?;
    }

    Ok(())
}

fn set_hard_rlimit(config: &SandboxConfig) -> Result<()> {
    macro_rules! direct_set {
        ($res:expr, $field:ident) => {
            if let Some($field) = config.$field {
                let $field = Rlim::from_raw($field.try_into()?);
                $res.set($field, $field)?;
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

fn cg_setup_child(config: &SandboxConfig, cg: &Cgroup) -> Result<()> {
    Cgroup::add_self_proc(cg.cpu()).context("failed to add self to cpu cgroup")?;
    Cgroup::add_self_proc(cg.memory()).context("failed to add self to memory cgroup")?;

    if let Some(memory_limit) = config.cg_limit_memory {
        Cgroup::write_type(cg.memory(), "memory.limit_in_bytes", memory_limit)
            .context("failed to set memory limit")?;
    }

    if let Some(pids_max) = config.cg_limit_max_pids {
        Cgroup::write_type(cg.pids(), "pids.max", pids_max)
            .context("failed to set max pids limit")?;
        Cgroup::add_self_proc(cg.pids()).context("failed to add self to pids cgroup")?;
    }

    Ok(())
}

fn cg_prepare_reset_metrics(cg: &Cgroup) -> Result<impl FnOnce() -> Result<()>> {
    let mut cpu = fs::File::create(&format!("{}/cpuacct.usage", cg.cpu()))?;
    let mut mem = fs::File::create(&format!("{}/memory.max_usage_in_bytes", cg.memory()))?;

    Ok(move || {
        write!(cpu, "0")?;
        write!(mem, "0")?;
        Ok(())
    })
}

fn set_id(config: &SandboxConfig) -> Result<()> {
    if let Some(gid) = config.gid.map(Gid::from_raw) {
        unistd::setgroups(&[gid]).context("failed to set groups")?;
        unistd::setresgid(gid, gid, gid).context("failed to set gid")?;
    }

    if let Some(uid) = config.uid.map(Uid::from_raw) {
        unistd::setresuid(uid, uid, uid).context("failed to set uid")?;
    }

    Ok(())
}
