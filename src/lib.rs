#![deny(clippy::all, clippy::cargo)]

#[macro_use]
mod utils;

mod cgroup_v1;
mod child;
mod cmd;
mod mount;
mod pipe;
mod proc;
mod run;
mod signal;

pub use crate::cmd::Command;

use crate::utils::RawFd;

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::Clap;
use crossbeam_utils::thread;
use memchr::memchr;
use serde::{Deserialize, Serialize};

pub fn run(config: &SandboxConfig) -> Result<SandboxOutput> {
    let handle = tokio::runtime::Handle::current();
    let run_in_tokio = move |config| {
        let _enter = handle.enter();
        crate::run::run(config)
    };
    thread::scope(|s| s.spawn(|_| run_in_tokio(config)).join().unwrap()).unwrap()
}

pub fn run_standalone(config: &SandboxConfig) -> Result<SandboxOutput> {
    crate::run::run(config)
}

#[derive(Debug, Default, Serialize, Deserialize, Clap)]
#[clap(
    version = clap::crate_version!(),
    author = clap::crate_authors!(),
    setting(clap::AppSettings::DeriveDisplayOrder),
)]
pub struct SandboxConfig {
    pub bin: PathBuf, // relative to chroot

    pub args: Vec<OsString>,

    #[clap(short = 'e', long)]
    pub env: Vec<OsString>,

    #[clap(short = 'c', long, value_name = "path")]
    pub chroot: Option<PathBuf>, // relative to cwd

    #[clap(long)]
    pub uid: Option<u32>,

    #[clap(long)]
    pub gid: Option<u32>,

    #[clap(long, value_name = "path")]
    pub stdin: Option<PathBuf>, // relative to chroot

    #[clap(long, value_name = "path")]
    pub stdout: Option<PathBuf>, // relative to chroot

    #[clap(long, value_name = "path")]
    pub stderr: Option<PathBuf>, // relative to chroot

    #[clap(long, value_name = "fd", conflicts_with = "stdin")]
    pub stdin_fd: Option<RawFd>,

    #[clap(long, value_name = "fd", conflicts_with = "stdout")]
    pub stdout_fd: Option<RawFd>,

    #[clap(long, value_name = "fd", conflicts_with = "stderr")]
    pub stderr_fd: Option<RawFd>,

    #[clap(short = 't', long, value_name = "milliseconds")]
    pub real_time_limit: Option<u64>,

    #[clap(long, value_name = "seconds")]
    pub rlimit_cpu: Option<u32>,

    #[clap(long, value_name = "bytes")]
    pub rlimit_as: Option<u64>,

    #[clap(long, value_name = "bytes")]
    pub rlimit_data: Option<u64>,

    #[clap(long, value_name = "bytes")]
    pub rlimit_fsize: Option<u64>,

    #[clap(long, value_name = "bytes")]
    pub cg_limit_memory: Option<u64>,

    #[clap(long, value_name = "count")]
    pub cg_limit_max_pids: Option<u32>,

    #[clap(
        long,
        value_name = "bindmount",
        parse(try_from_os_str = BindMount::try_from_os_str)
    )]
    pub bindmount_rw: Vec<BindMount>,

    #[clap(
        short = 'b',
        long,
        value_name = "bindmount",
        parse(try_from_os_str = BindMount::try_from_os_str)
    )]
    pub bindmount_ro: Vec<BindMount>,

    #[clap(
        long,
        value_name = "path",
        min_values = 0,
        require_equals = true,
        default_missing_value = "/proc"
    )]
    pub mount_proc: Option<PathBuf>, // absolute (affected by chroot)

    #[clap(
        long,
        value_name = "path",
        min_values = 0,
        require_equals = true,
        default_missing_value = "/tmp"
    )]
    pub mount_tmpfs: Option<PathBuf>, // absolute (affected by chroot)

    #[clap(long, value_name = "prio")]
    pub priority: Option<i8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BindMount {
    pub src: PathBuf, // absolute
    pub dst: PathBuf, // absolute (affected by chroot)
}

impl BindMount {
    fn try_from_os_str(s: &OsStr) -> Result<Self, String> {
        let (src, dst) = match memchr(b':', s.as_bytes()) {
            Some(idx) => {
                let src = OsStr::from_bytes(&s.as_bytes()[..idx]);
                let dst = OsStr::from_bytes(&s.as_bytes()[idx + 1..]);
                if src.is_empty() || dst.is_empty() {
                    return Err("invalid bind mount format".into());
                }
                (src, dst)
            }
            None => (s, s),
        };
        Ok(BindMount {
            src: src.into(),
            dst: dst.into(),
        })
    }

    pub fn new_same(src: PathBuf) -> Self {
        Self {
            dst: src.clone(),
            src,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SandboxOutput {
    pub code: i32,
    pub signal: i32,

    pub real_time: u64, // milliseconds
    pub sys_time: u64,  // milliseconds
    pub user_time: u64, // milliseconds

    pub memory: u64, // KiB
}

impl SandboxOutput {
    pub fn is_success(&self) -> bool {
        self.code == 0 && self.signal == 0
    }
}

impl SandboxConfig {
    pub fn to_cli_cmd(&self) -> process::Command {
        let mut cmd = process::Command::new("carapace");

        macro_rules! push {
            (@os_str $opt: literal, $f: ident) => {
                if let Some(ref $f) = self.$f {
                    cmd.arg($opt).arg($f);
                }
            };
            (@num $opt: literal, $f: ident) => {
                if let Some(ref $f) = self.$f {
                    cmd.arg($opt).arg($f.to_string());
                }
            };
            (@bindmount $opt: literal, $f: ident) => {
                for mnt in &self.$f {
                    cmd.arg($opt);
                    let mut s: OsString = mnt.src.as_os_str().into();
                    if mnt.src != mnt.dst {
                        s.push(":");
                        s.push(&mnt.dst);
                    }
                    cmd.arg(s);
                }
            };
            (@os_str @opt_arg $opt: literal, $f: ident) => {
                if let Some(ref $f) = self.$f {
                    let mut s: OsString = $opt.into();
                    s.push("=");
                    s.push($f);
                    cmd.arg(s);
                }
            };
            (@os_str @multi $opt: literal, $f: ident) => {
                for $f in &self.$f {
                    cmd.arg($opt).arg($f);
                }
            };
        }

        push!(@num "--uid", uid);
        push!(@num "--gid", gid);

        push!(@num "--rlimit-cpu", rlimit_cpu);
        push!(@num "--rlimit-as", rlimit_as);
        push!(@num "--rlimit-data", rlimit_data);
        push!(@num "--rlimit-fsize", rlimit_fsize);

        push!(@num "--cg-limit-memory", cg_limit_memory);
        push!(@num "--cg-limit-max-pids", cg_limit_max_pids);

        push!(@bindmount "--bindmount-rw", bindmount_rw);
        push!(@bindmount "-b", bindmount_ro);

        push!(@os_str @opt_arg "--mount-proc", mount_proc);
        push!(@os_str @opt_arg "--mount-tmpfs", mount_tmpfs);

        push!(@num "--priority", priority);

        push!(@num "--stdin-fd", stdin_fd);
        push!(@num "--stdout-fd", stdout_fd);
        push!(@num "--stderr-fd", stderr_fd);

        push!(@num "-t", real_time_limit);

        push!(@os_str "-c", chroot);

        push!(@os_str @multi "-e", env);

        push!(@os_str "--stdin", stdin);
        push!(@os_str "--stdout", stdout);
        push!(@os_str "--stderr", stderr);

        cmd.arg("--");
        cmd.arg(&self.bin);
        cmd.args(&self.args);

        cmd
    }
}
