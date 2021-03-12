use crate::utils::libc_call;

use std::mem::{self, ManuallyDrop};

use std::panic::{self, AssertUnwindSafe};

use std::{io, ptr};

use nix::sched::CloneFlags;

use nix::unistd::Pid;

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
