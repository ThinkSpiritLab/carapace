use std::io;
use std::mem::MaybeUninit;

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
