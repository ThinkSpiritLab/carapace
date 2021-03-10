use std::fs::File;
use std::io::Write as _;
use std::str::FromStr;
use std::{fmt, fs, io};

use anyhow::{Context, Result};
use nix::sys::stat::Mode;
use nix::unistd::{self, AccessFlags};
use tracing::trace;

pub struct Cgroup {
    cpu: String,
    memory: String,
    pids: String,
}

impl Cgroup {
    pub fn create(name: &str) -> Result<Self> {
        trace!(?name, "create cgroup");
        let cpu = format!("/sys/fs/cgroup/cpu/{}", name);
        let memory = format!("/sys/fs/cgroup/memory/{}", name);
        let pids = format!("/sys/fs/cgroup/pids/{}", name);
        Self::ensure_dir(&cpu)?;
        Self::ensure_dir(&memory)?;
        Self::ensure_dir(&pids)?;
        Ok(Self { cpu, memory, pids })
    }

    pub fn cpu(&self) -> &str {
        &self.cpu
    }

    pub fn memory(&self) -> &str {
        &self.memory
    }

    pub fn pids(&self) -> &str {
        &self.pids
    }

    pub fn ensure_dir(cg_dir: &str) -> Result<()> {
        if unistd::access(cg_dir, AccessFlags::F_OK).is_ok() {
            return Ok(());
        }

        unistd::mkdir(cg_dir, Mode::from_bits_truncate(0o755))
            .with_context(|| format!("fail to create cgroup directory: {}", cg_dir))?;

        Ok(())
    }

    pub fn remove_dir(cg_dir: &str) -> io::Result<()> {
        fs::remove_dir(cg_dir)
    }

    pub fn add_self_proc(cg_dir: &str) -> io::Result<()> {
        let path = format!("{}/cgroup.procs", cg_dir);
        let mut file = fs::OpenOptions::new().append(true).open(path)?;
        write!(file, "0")?;
        Ok(())
    }

    pub fn write_type(cg_dir: &str, file: &str, content: impl fmt::Display) -> io::Result<()> {
        let path = format!("{}/{}", cg_dir, file);
        let mut file = File::create(&path)?;
        write!(file, "{}", content)?;
        Ok(())
    }

    pub fn read_type<T>(cg_dir: &str, file: &str) -> Result<T>
    where
        T: FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        let path = format!("{}/{}", cg_dir, file);
        let content = fs::read_to_string(path)?;
        Ok(content.trim_end().parse::<T>()?)
    }

    pub fn read_string(cg_dir: &str, file: &str) -> io::Result<String> {
        let path = format!("{}/{}", cg_dir, file);
        let content = fs::read_to_string(path)?;
        Ok(content)
    }
}
