use std::mem::{ManuallyDrop, MaybeUninit};
use std::panic::{self, AssertUnwindSafe};
use std::{io, ptr};

use nix::sched::CloneFlags;
use nix::unistd::Pid;
use tracing::trace;

pub type RawFd = std::os::unix::io::RawFd;

pub fn libc_call(f: impl FnOnce() -> i32) -> io::Result<u32> {
    let ret = f();
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(ret as u32)
}

pub fn wait4(child_pid: Pid) -> io::Result<(i32, libc::rusage)> {
    let pid = child_pid.as_raw();
    let mut status: i32 = 0;
    let mut rusage: MaybeUninit<libc::rusage> = MaybeUninit::zeroed();

    loop {
        let ret = libc_call(|| unsafe {
            libc::wait4(pid, &mut status, libc::WUNTRACED, rusage.as_mut_ptr())
        })?;

        trace!("wait4 ret = {}, status = {}", ret, status);

        if ret > 0 {
            break;
        }
    }

    unsafe { Ok((status, rusage.assume_init())) }
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
