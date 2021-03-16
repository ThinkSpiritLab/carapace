use crate::cgroup_v1::Cgroup;
use crate::mount::{bind_mount, make_root_private, mount_proc, mount_tmpfs};
use crate::seccomp;
use crate::utils::{self, RawFd};
use crate::SandboxConfig;

use std::borrow::Cow;
use std::convert::{Infallible, TryInto};
use std::ffi::{CString, OsString};
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::{env, fs, io, ptr};

use anyhow::{Context, Result};
use nix::fcntl::{self, OFlag};
use nix::sys::stat::Mode;
use nix::unistd::{self, AccessFlags, Gid, Uid};
use path_absolutize::Absolutize;
use rlimit::{Resource, Rlim};

pub fn run_child(config: &SandboxConfig, cgroup: &Cgroup) -> Result<Infallible> {
    unsafe { path_absolutize::update_cwd() };

    do_mount(&config)?;

    let exec = prepare_execve_args(config)?;

    cg_setup_child(config, cgroup).context("failed to setup cgroup")?;

    let reset: _ = cg_prepare_reset_metrics(cgroup).context("failed to prepare cgroup metrics")?;

    if let Some(ref new_root) = config.chroot {
        unistd::chroot(new_root)
            .and_then(|_| unistd::chdir("/"))
            .context("failed to chroot")?;
    }

    set_hard_rlimit(config)?;

    if let Some(prio) = config.priority {
        utils::libc_call(|| unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, prio as _) })
            .context("failed to set priority")?;
    }

    redirect_stdio(config)?;

    unistd::access(&config.bin, AccessFlags::F_OK)
        .with_context(|| format!("failed to access file: path = {}", config.bin.display()))?;

    if config.seccomp_forbid_ipc {
        let mut seccomp_ctx = seccomp::Context::new();
        seccomp_ctx.forbid_ipc();
        seccomp_ctx.install()?;
    }

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
    // NOTE:
    // I accidentally ran into a deadlock problem here.
    // It seems that the deadlock occurs in `__nptl_setxid` (found by `strace -k`)
    // So I use direct syscalls to work around.
    //
    // Env:  rustc 1.50.0, glibc 2.27
    // Time: 2021-03-13

    if let Some(gid) = config.gid.map(Gid::from_raw) {
        setresgid(gid, gid, gid).context("failed to set gid")?;
        setgroups(&[gid]).context("failed to set groups")?;
    }

    if let Some(uid) = config.uid.map(Uid::from_raw) {
        setresuid(uid, uid, uid).context("failed to set uid")?;
    }

    Ok(())
}

fn setgroups(groups: &[Gid]) -> io::Result<()> {
    unsafe {
        let size: libc::c_long = groups.len() as _;
        let ptr: libc::c_long = groups.as_ptr() as _;

        let ret = libc::syscall(libc::SYS_setgroups, size, ptr);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

fn setresgid(rgid: Gid, egid: Gid, sgid: Gid) -> io::Result<()> {
    unsafe {
        let rgid: libc::c_long = rgid.as_raw() as _;
        let egid: libc::c_long = egid.as_raw() as _;
        let sgid: libc::c_long = sgid.as_raw() as _;

        let ret = libc::syscall(libc::SYS_setresgid, rgid, egid, sgid);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

fn setresuid(ruid: Uid, euid: Uid, suid: Uid) -> io::Result<()> {
    unsafe {
        let ruid: libc::c_long = ruid.as_raw() as _;
        let euid: libc::c_long = euid.as_raw() as _;
        let suid: libc::c_long = suid.as_raw() as _;

        let ret = libc::syscall(libc::SYS_setresuid, ruid, euid, suid);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
