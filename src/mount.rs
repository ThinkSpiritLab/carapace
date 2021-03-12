use crate::utils::{is_dir, libc_call, with_c_str};

use std::ffi::{CStr, CString};

use std::os::unix::ffi::OsStrExt;

use std::path::Path;
use std::{fs, io, ptr};

use anyhow::Result;
use nix::fcntl::{self, OFlag};

use nix::sys::stat::Mode;
use nix::unistd::{self, AccessFlags};

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

pub fn mount_proc(dst: &Path) -> io::Result<()> {
    with_c_str(dst.as_os_str().as_bytes(), |dst| {
        let src = b"none\0".as_ptr().cast();
        let fstype = b"proc\0".as_ptr().cast();
        libc_call(|| unsafe { libc::mount(src, dst.as_ptr(), fstype, 0, ptr::null()) })?;
        Ok(())
    })
}

pub fn mount_tmpfs(dst: &Path) -> io::Result<()> {
    with_c_str(dst.as_os_str().as_bytes(), |dst| {
        let src = b"none\0".as_ptr().cast();
        let fstype = b"tmpfs\0".as_ptr().cast();
        libc_call(|| unsafe { libc::mount(src, dst.as_ptr(), fstype, 0, ptr::null()) })?;
        Ok(())
    })
}

/// prevent propagation of mount events to other mount namespaces
/// https://man7.org/linux/man-pages/man7/mount_namespaces.7.html
pub fn make_root_private() -> io::Result<()> {
    libc_call(|| unsafe {
        let flags = libc::MS_PRIVATE | libc::MS_REC;
        let dst = b"/\0".as_ptr().cast();
        let null = ptr::null();
        libc::mount(null, dst, null, flags, null.cast())
    })?;
    Ok(())
}
