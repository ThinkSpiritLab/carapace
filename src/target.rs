mod limit;
mod status;

pub use self::limit::TargetLimit;
pub use self::status::TargetStatus;

use std::fs::File;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

pub struct Target {
    pub bin_path: PathBuf,
    pub arguments: Vec<String>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub input_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub error_path: Option<PathBuf>,
    pub limit: TargetLimit,
}

impl Target {
    pub fn new(bin_path: PathBuf) -> Self {
        Self {
            bin_path,
            arguments: vec![],
            uid: None,
            gid: None,
            input_path: None,
            output_path: None,
            error_path: None,
            limit: TargetLimit {
                max_real_time: None,
            },
        }
    }
}

impl Target {
    pub fn run(&self) -> Result<TargetStatus, std::io::Error> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args(&self.arguments);

        self.uid.map(|uid| cmd.uid(uid));
        self.gid.map(|gid| cmd.gid(gid));

        if let Some(ref input_path) = self.input_path {
            cmd.stdin(File::open(input_path)?);
        }

        if let Some(ref output_path) = self.output_path {
            cmd.stdout(File::create(output_path)?);
        }
        if let Some(ref error_path) = self.error_path {
            cmd.stderr(File::create(error_path)?);
        }

        let mut child = cmd.spawn()?;

        let raw_pid = child.id() as i32;
        let pid = Pid::from_raw(raw_pid);

        self.limit.max_real_time.map(|max_real_time| {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(max_real_time));
                let _ = kill(pid, Signal::SIGKILL);
            })
        });

        let status = child.wait()?;

        Ok(TargetStatus {
            code: status.code(),
            signal: None,
        })
    }
}
