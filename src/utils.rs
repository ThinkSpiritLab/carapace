use std::ffi::{CStr, CString};
use std::mem::{self, ManuallyDrop};
use std::os::unix::ffi::OsStrExt;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::{fs, io, ptr};

use anyhow::Result;
use nix::fcntl::{self, OFlag};
use nix::sched::CloneFlags;
use nix::sys::stat::Mode;
use nix::unistd::{self, AccessFlags, Pid};
use nix::NixPath;

pub type RawFd = std::os::unix::io::RawFd;

pub fn libc_call(f: impl FnOnce() -> i32) -> io::Result<u32> {
    let ret = f();
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(ret as u32)
}

pub fn wait_child(child_pid: Pid) -> io::Result<(i32, i32)> {
    unsafe fn waitid(
        pid: u32,
        info: &mut libc::siginfo_t,
        nohang: bool,
    ) -> io::Result<Option<(i32, i32)>> {
        let options = if nohang {
            libc::WEXITED | libc::WNOHANG
        } else {
            libc::WEXITED
        };

        libc_call(|| libc::waitid(libc::P_PID, pid, info, options))?;

        if info.si_pid() > 0 {
            if info.si_code == libc::CLD_EXITED {
                Ok(Some((info.si_status(), 0)))
            } else {
                Ok(Some((0, info.si_status())))
            }
        } else {
            Ok(None)
        }
    }

    let pid = child_pid.as_raw() as u32;
    let mut info: libc::siginfo_t = unsafe { mem::zeroed() };

    loop {
        if let Some(ret) = unsafe { waitid(pid, &mut info, false)? } {
            return Ok(ret);
        }
    }
}

pub unsafe fn clone_proc<F: FnOnce() -> libc::c_int>(
    cb: F,
    stack: &mut [u8],
    flags: CloneFlags,
    signal: libc::c_int,
) -> io::Result<Pid> {
    extern "C" fn child_fn<F>(data: *mut libc::c_void) -> libc::c_int
    where
        F: FnOnce() -> libc::c_int + Sized,
    {
        let f = unsafe { ptr::read(data.cast::<F>()) };
        panic::catch_unwind(AssertUnwindSafe(|| f())).unwrap_or(101)
    }

    let mut f = ManuallyDrop::new(cb);

    let data: *mut F = &mut *f;
    let stack_top = stack.as_mut_ptr().add(stack.len());

    let ret = libc_call(|| {
        libc::clone(
            child_fn::<F>,
            stack_top.cast(),
            flags.bits() | signal,
            data.cast(),
        )
    });

    Ok(Pid::from_raw(ret? as _))
}

pub fn is_dir(path: &(impl NixPath + ?Sized)) -> nix::Result<bool> {
    nix::sys::stat::stat(path).map(|stat| stat.st_mode & libc::S_IFMT == libc::S_IFDIR)
}

pub fn bind_mount(src_path: &Path, dst_path: &Path, recursive: bool, readonly: bool) -> Result<()> {
    let src: &CStr = &CString::new(src_path.as_os_str().as_bytes())?;
    let dst: &CStr = &CString::new(dst_path.as_os_str().as_bytes())?;

    let src_is_dir = is_dir(src)?;

    let dst_exists = unistd::access(dst, AccessFlags::F_OK).is_ok();

    if !dst_exists {
        if src_is_dir {
            fs::create_dir_all(dst_path)?;
        } else {
            if let Some(parent_dir) = dst_path.parent() {
                fs::create_dir_all(parent_dir)?;
            }
            let fd = fcntl::open(
                dst,
                OFlag::O_CREAT | OFlag::O_RDONLY | OFlag::O_CLOEXEC,
                Mode::from_bits_truncate(0o644),
            )?;
            let _ = unistd::close(fd);
        }
    }

    let do_mount = |flags| unsafe {
        libc_call(|| libc::mount(src.as_ptr(), dst.as_ptr(), ptr::null(), flags, ptr::null()))
    };

    do_mount(if recursive {
        libc::MS_BIND | libc::MS_REC
    } else {
        libc::MS_BIND
    })?;

    if readonly {
        do_mount(libc::MS_REMOUNT | libc::MS_BIND | libc::MS_RDONLY)?;
    }

    Ok(())
}
