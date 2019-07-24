mod target;

pub use self::target::{SeccompRule, Target, TargetLimit, TargetRule, TargetStatus};

pub use rlimit::RLIM_INFINITY;
pub use syscallz;

#[inline(always)]
pub(crate) fn check_os_error(ret: libc::c_int) -> std::io::Result<libc::c_int> {
    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    } else {
        Ok(ret)
    }
}
