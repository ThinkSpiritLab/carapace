use anyhow::Result;
use seccomp_sys::*;

pub struct Context(*mut scmp_filter_ctx);

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { seccomp_release(self.0) }
    }
}

impl Context {
    pub fn new() -> Context {
        let ctx = unsafe { seccomp_init(SCMP_ACT_ALLOW) };
        Self(ctx)
    }

    pub fn forbid_ipc(&mut self) {
        let kill_syscalls = |nrs: &[libc::c_long]| unsafe {
            for &nr in nrs {
                seccomp_rule_add(self.0, SCMP_ACT_KILL_PROCESS, nr as _, 0);
            }
        };
        kill_syscalls(&[
            libc::SYS_msgget,
            libc::SYS_semget,
            libc::SYS_shmget,
            libc::SYS_mq_open,
        ]);
    }

    pub fn install(self) -> Result<()> {
        unsafe {
            let ret = seccomp_load(self.0);
            if ret < 0 {
                anyhow::bail!("failed to load seccomp: ret = {}", ret)
            }
        }
        Ok(())
    }
}
