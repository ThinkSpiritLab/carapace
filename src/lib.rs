#![deny(clippy::all)]

mod cgroup_v1;
mod pipe;
mod run;
mod signal;
mod utils;

pub use crate::run::run;

use crate::utils::RawFd;

use std::ffi::OsString;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use structopt::clap;

#[derive(Debug, Default, Serialize, Deserialize, StructOpt)]
#[structopt(setting = clap::AppSettings::DeriveDisplayOrder)]
pub struct SandboxConfig {
    pub bin: PathBuf,

    pub args: Vec<OsString>,

    #[structopt(short = "e", long)]
    pub env: Vec<OsString>,

    #[structopt(long, value_name = "path")]
    pub chroot: Option<PathBuf>,

    #[structopt(long)]
    pub uid: Option<u32>,

    #[structopt(long)]
    pub gid: Option<u32>,

    #[structopt(long, value_name = "path")]
    pub stdin: Option<PathBuf>,

    #[structopt(long, value_name = "path")]
    pub stdout: Option<PathBuf>,

    #[structopt(long, value_name = "path")]
    pub stderr: Option<PathBuf>,

    #[structopt(long, value_name = "fd", conflicts_with = "stdin")]
    pub stdin_fd: Option<RawFd>,

    #[structopt(long, value_name = "fd", conflicts_with = "stdout")]
    pub stdout_fd: Option<RawFd>,

    #[structopt(long, value_name = "fd", conflicts_with = "stderr")]
    pub stderr_fd: Option<RawFd>,

    #[structopt(short = "t", long, value_name = "milliseconds")]
    pub real_time_limit: Option<u64>,

    #[structopt(long, value_name = "seconds")]
    pub rlimit_cpu: Option<u32>,

    #[structopt(long, value_name = "bytes")]
    pub rlimit_as: Option<u64>,

    #[structopt(long, value_name = "bytes")]
    pub rlimit_data: Option<u64>,

    #[structopt(long, value_name = "bytes")]
    pub rlimit_fsize: Option<u64>,

    #[structopt(long, value_name = "bytes")]
    pub cg_limit_memory: Option<u64>,

    #[structopt(long, value_name = "count")]
    pub cg_limit_max_pids: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SandboxOutput {
    pub code: i32,
    pub signal: i32,
    pub status: i32,

    pub real_time: u64, // milliseconds
    pub sys_time: u64,  // milliseconds
    pub user_time: u64, // milliseconds

    pub memory: u64, // KiB
}
