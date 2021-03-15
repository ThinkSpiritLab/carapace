use crate::{SandboxConfig, SandboxOutput};

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use anyhow::Result;

pub struct Command {
    pub config: SandboxConfig,
}

impl Command {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            config: SandboxConfig {
                bin: bin.into(),
                ..Default::default()
            },
        }
    }

    pub fn run(&self) -> Result<SandboxOutput> {
        crate::run(&self.config)
    }

    pub fn arg(&mut self, a: impl Into<OsString>) -> &mut Self {
        self.config.args.push(a.into());
        self
    }

    pub fn arg_if(&mut self, cond: bool, a: impl Into<OsString>) -> &mut Self {
        if cond {
            self.arg(a)
        } else {
            self
        }
    }

    pub fn inherit_env(&mut self, k: impl Into<OsString>) -> &mut Self {
        self.config.env.push(k.into()); // TODO: check b'=' and b'\0' ?
        self
    }

    pub fn add_env(&mut self, k: impl Into<OsString>, v: impl AsRef<OsStr>) -> &mut Self {
        let mut e: OsString = k.into();
        e.push(OsStr::from_bytes(b"="));
        e.push(v.as_ref());
        self.config.env.push(e); // TODO: check b'=' and b'\0' ?
        self
    }

    pub fn bindmount_ro(&mut self, src: impl Into<PathBuf>, dst: impl Into<PathBuf>) -> &mut Self {
        self.config.bindmount_ro.push(crate::BindMount {
            src: src.into(),
            dst: dst.into(),
        });
        self
    }

    pub fn chroot(&mut self, chroot: impl Into<PathBuf>) -> &mut Self {
        self.config.chroot = Some(chroot.into());
        self
    }

    pub fn stdio(
        &mut self,
        stdin: impl Into<PathBuf>,
        stdout: impl Into<PathBuf>,
        stderr: impl Into<PathBuf>,
    ) -> &mut Self {
        self.config.stdin = Some(stdin.into());
        self.config.stdout = Some(stdout.into());
        self.config.stderr = Some(stderr.into());
        self
    }

    pub fn mount_proc(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.config.mount_proc = Some(path.into());
        self
    }
}
