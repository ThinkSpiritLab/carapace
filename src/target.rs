mod limit;
mod status;

pub use self::limit::TargetLimit;
pub use self::status::TargetStatus;

use std::fs::File;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::{Duration, SystemTime};

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
                max_stack_size: None,
                max_cpu_time: None,
                max_process_number: None,
                max_output_size: None,
                max_memory: None,
            },
        }
    }
}

impl Target {
    fn spawn(&self) -> std::io::Result<Pid> {
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

        unsafe {
            let limit = Box::new(self.limit.clone());
            cmd.pre_exec(move || limit.apply_rlimit());
        }

        let child = cmd.spawn()?;
        let pid = Pid::from_raw(child.id() as i32);

        self.limit.max_real_time.map(|max_real_time| {
            thread::spawn(move || {
                thread::sleep(Duration::from_micros(max_real_time));
                let _ = kill(pid, Signal::SIGKILL);
            })
        });

        Ok(pid)
    }

    fn wait(&self, pid: Pid) -> std::io::Result<TargetStatus> {
        let mut status = unsafe { std::mem::zeroed::<libc::c_int>() };
        let mut ru = unsafe { std::mem::zeroed::<libc::rusage>() };
        let t0 = SystemTime::now();

        let p = unsafe {
            libc::wait4(
                pid.as_raw(),
                &mut status as *mut libc::c_int,
                libc::WSTOPPED,
                &mut ru as *mut libc::rusage,
            )
        };

        if p == -1 {
            use std::io::{Error, ErrorKind};
            return Err(Error::from(ErrorKind::Other));
        }

        let status = ExitStatus::from_raw(status);

        let code = status.code();
        let signal = status.signal().map(|s| Signal::from_c_int(s).unwrap());
        let real_time = t0.elapsed().unwrap().as_micros() as u64;
        let user_time = (ru.ru_utime.tv_sec as u64 * 1000_000) + (ru.ru_utime.tv_usec as u64);
        let sys_time = (ru.ru_stime.tv_sec as u64 * 1000_000) + (ru.ru_stime.tv_usec as u64);
        let memory = ru.ru_maxrss as u64;

        Ok(TargetStatus {
            code,
            signal,
            real_time,
            user_time,
            sys_time,
            memory,
        })
    }
}

impl Target {
    pub fn run(&self) -> std::io::Result<TargetStatus> {
        self.spawn().and_then(|pid| self.wait(pid))
    }
}
