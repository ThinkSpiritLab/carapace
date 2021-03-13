use crate::utils::libc_call;

use std::io::{Read, Write};
use std::os::unix::io::RawFd;
use std::os::unix::prelude::FromRawFd;
use std::{fs, io, mem};

pub struct PipeTx(RawFd);
pub struct PipeRx(RawFd);

impl Drop for PipeTx {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.0) };
    }
}

impl Drop for PipeRx {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.0) };
    }
}

pub fn create() -> io::Result<(PipeTx, PipeRx)> {
    let mut sv = [0, 0];
    libc_call(|| unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
            0,
            sv.as_mut_ptr(),
        )
    })?;
    let rx = PipeRx(sv[0]);
    let tx = PipeTx(sv[1]);
    Ok((tx, rx))
}

impl PipeTx {
    pub fn write_error(self, err: anyhow::Error) -> io::Result<()> {
        let mut buf = Vec::new();
        write!(buf, "{:?}", err).unwrap();
        unsafe {
            let mut file = fs::File::from_raw_fd(self.0);
            let ret1 = file.write_all(&buf);
            let ret2 = file.flush();
            mem::forget(file);
            ret1.and(ret2)?;
        }
        Ok(())
    }
}

impl PipeRx {
    pub fn read_result(self) -> io::Result<anyhow::Result<()>> {
        let mut buf = String::new();
        unsafe {
            let mut file = fs::File::from_raw_fd(self.0);
            let ret = file.read_to_string(&mut buf);
            mem::forget(file);
            ret?;
        }
        if buf.is_empty() {
            Ok(Ok(()))
        } else {
            Ok(Err(anyhow::Error::msg(buf)))
        }
    }
}
