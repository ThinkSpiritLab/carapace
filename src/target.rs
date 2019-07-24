mod limit;
mod rule;
mod status;

pub use self::limit::TargetLimit;
pub use self::rule::{SeccompRule, TargetRule};
pub use self::status::TargetStatus;

use crate::check_os_error;

use std::ffi::{CString, NulError};
use std::fs::File;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::thread;
use std::time::SystemTime;

use syscallz::{Action, Cmp, Comparator, Syscall};

pub struct Target {
    pub bin_path: CString,
    pub args: Vec<CString>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub input_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub error_path: Option<PathBuf>,
    pub limit: TargetLimit,
    pub rule: TargetRule,
    pub allow_target_execve: bool,
}

impl Target {
    pub fn new(bin_path: &str) -> Result<Self, NulError> {
        Ok(Self {
            bin_path: CString::new(bin_path)?,
            args: vec![],
            uid: None,
            gid: None,
            input_path: None,
            output_path: None,
            error_path: None,
            limit: TargetLimit::new(),
            rule: TargetRule::new(),
            allow_target_execve: true,
        })
    }
}

impl Target {
    unsafe fn apply_settings(&self, extra_rules: &[SeccompRule]) -> std::io::Result<()> {
        if let Some(uid) = self.uid {
            check_os_error(libc::setuid(uid))?;
        }

        if let Some(gid) = self.gid {
            check_os_error(libc::setgid(gid))?;
        }

        if let Some(ref input_path) = self.input_path {
            let input_fd = File::open(input_path)?.as_raw_fd();
            let stdin_fd = libc::STDIN_FILENO;
            check_os_error(libc::dup2(input_fd, stdin_fd))?;
        }

        if let Some(ref output_path) = self.output_path {
            let output_fd = File::create(output_path)?.as_raw_fd();
            let stdout_fd = libc::STDOUT_FILENO;
            check_os_error(libc::dup2(output_fd, stdout_fd))?;
        }

        if let Some(ref error_path) = self.error_path {
            let error_fd = File::create(error_path)?.as_raw_fd();
            let stderr_fd = libc::STDERR_FILENO;
            check_os_error(libc::dup2(error_fd, stderr_fd))?;
        }

        self.limit.apply_rlimit()?;
        self.rule.apply_seccomp(extra_rules)?;
        Ok(())
    }

    unsafe fn spawn(&self) -> std::io::Result<libc::pid_t> {
        let argv = {
            let mut argv: Vec<*const libc::c_char> = Vec::with_capacity(self.args.len() + 2);
            argv.push(self.bin_path.as_ptr());
            argv.extend(self.args.iter().map(|s| s.as_ptr()));
            argv.push(libc::PT_NULL as *const libc::c_char);
            argv
        };

        let extra_rules = {
            let execve_rule = {
                let act = self.rule.default_action.unwrap_or(Action::Allow);
                if let Action::Allow = act {
                    if self.allow_target_execve {
                        None
                    } else {
                        Some(SeccompRule {
                            action: Action::Kill,
                            syscall: Syscall::execve,
                            comparators: vec![Comparator::new(0, Cmp::Ne, argv[0] as u64, 0)],
                        })
                    }
                } else {
                    let mut rule = SeccompRule {
                        action: Action::Allow,
                        syscall: Syscall::execve,
                        comparators: vec![],
                    };

                    if !self.allow_target_execve {
                        rule.comparators
                            .push(Comparator::new(0, Cmp::Eq, argv[0] as u64, 0));
                    }

                    Some(rule)
                }
            };
            match execve_rule {
                None => vec![],
                Some(rule) => vec![rule],
            }
        };

        // create pipe: child -> parent
        let (rx_fd, tx_fd) = {
            let mut fds: [RawFd; 2] = [0; 2];
            check_os_error(libc::pipe(fds.as_mut_ptr()))?;
            (fds[0], fds[1])
        };

        let ret = check_os_error(libc::fork())?;

        if ret == 0 {
            // child process
            let _ = libc::close(rx_fd);

            let mut errno = match self.apply_settings(extra_rules.as_slice()) {
                Ok(_) => 0,
                Err(err) => err.raw_os_error().unwrap(), // assert: `err` is raw os error
            };

            // send errno
            let bytes = (errno as i32).to_ne_bytes();
            let _ = libc::write(tx_fd, bytes.as_ptr() as *const libc::c_void, bytes.len());
            let _ = libc::close(tx_fd);

            if errno == 0 {
                libc::execvp(argv[0], argv.as_ptr()); // child process ends here on success
                errno = *libc::__errno_location();
            }

            libc::exit(errno)
        } else {
            // parent process
            let _ = libc::close(tx_fd);

            let pid = ret;

            // receive errno
            let mut bytes: [u8; 4] = [0; 4];
            let ret = libc::read(rx_fd, bytes.as_mut_ptr() as *mut libc::c_void, bytes.len());
            let _ = libc::close(rx_fd);

            if ret < 0 {
                let errno = *libc::__errno_location();
                let _ = libc::kill(pid, libc::SIGKILL);
                return Err(std::io::Error::from_raw_os_error(errno)); // error of libc::read
            }

            let errno = i32::from_ne_bytes(bytes);

            if errno == 0 {
                Ok(pid)
            } else {
                Err(std::io::Error::from_raw_os_error(errno))
            }
        }
    }

    unsafe fn wait(&self, pid: libc::pid_t) -> std::io::Result<TargetStatus> {
        if let Some(max_real_time) = self.limit.max_real_time {
            thread::Builder::new().spawn(move || {
                let _ = libc::usleep(max_real_time);
                let _ = libc::kill(pid, libc::SIGKILL);
            })?;
        }

        let mut status = std::mem::zeroed::<libc::c_int>();
        let mut ru = std::mem::zeroed::<libc::rusage>();
        let t0 = SystemTime::now();

        let ret = libc::wait4(
            pid,
            &mut status as *mut libc::c_int,
            libc::WSTOPPED,
            &mut ru as *mut libc::rusage,
        );
        if ret < 0 {
            let errno = *libc::__errno_location();
            let _ = libc::kill(pid, libc::SIGKILL);
            return Err(std::io::Error::from_raw_os_error(errno)); // error of libc::wait4
        }

        let real_time = t0.elapsed().unwrap().as_micros() as u64;
        let code;
        let signal;

        let exited = libc::WIFEXITED(status);
        if exited {
            code = Some(libc::WEXITSTATUS(status));
            signal = None;
        } else {
            code = None;
            signal = Some(libc::WTERMSIG(status));
        }
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
        unsafe { self.wait(self.spawn()?) }
    }
}
