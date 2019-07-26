use super::limit::TargetLimit;
use super::rule::{SeccompRule, TargetRule};
use super::status::TargetStatus;

use std::ffi::{CString, NulError};
use std::io::{Error as IOError, Result as IOResult};
use std::thread;
use std::time::SystemTime;

use libc::{c_char, c_int};
use syscallz::{Action, Cmp, Comparator, Syscall};

pub struct Target {
    pub bin_path: CString,
    pub args: Vec<CString>,
    pub envs: Vec<CString>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub stdin: Option<CString>,
    pub stdout: Option<CString>,
    pub stderr: Option<CString>,
    pub limit: TargetLimit,
    pub rule: TargetRule,
    pub forbid_target_execve: bool,
    pub forbid_inherited_env: bool,
}

impl Target {
    pub fn from_bin_path(bin_path: CString) -> Self {
        Self {
            bin_path,
            args: vec![],
            envs: vec![],
            uid: None,
            gid: None,
            stdin: None,
            stdout: None,
            stderr: None,
            limit: TargetLimit::new(),
            rule: TargetRule::new(),
            forbid_target_execve: false,
            forbid_inherited_env: false,
        }
    }

    pub fn new(bin_path: &str) -> Result<Self, NulError> {
        Ok(Self::from_bin_path(CString::new(bin_path)?))
    }

    pub fn add_arg(&mut self, arg: &str) -> Result<(), NulError> {
        Ok(self.args.push(CString::new(arg)?))
    }

    pub fn add_env(&mut self, env: &str) -> Result<(), NulError> {
        Ok(self.envs.push(CString::new(env)?))
    }

    pub fn set_stdin(&mut self, input_path: &str) -> Result<(), NulError> {
        Ok(self.stdin = Some(CString::new(input_path)?))
    }

    pub fn set_stdout(&mut self, output_path: &str) -> Result<(), NulError> {
        Ok(self.stdout = Some(CString::new(output_path)?))
    }

    pub fn set_stderr(&mut self, error_path: &str) -> Result<(), NulError> {
        Ok(self.stderr = Some(CString::new(error_path)?))
    }
}

#[inline(always)]
unsafe fn get_errno() -> c_int {
    *libc::__errno_location()
}

#[inline(always)]
fn check_os_error(ret: libc::c_int) -> IOResult<libc::c_int> {
    if ret < 0 {
        return Err(IOError::last_os_error());
    } else {
        Ok(ret)
    }
}

unsafe fn open_read_fd(path: *const c_char) -> c_int {
    use libc::{AT_FDCWD, O_RDONLY};
    libc::openat(AT_FDCWD, path, O_RDONLY, 0o666)
}

unsafe fn open_write_fd(path: *const c_char) -> c_int {
    use libc::{AT_FDCWD, O_CREAT, O_TRUNC, O_WRONLY};
    libc::openat(AT_FDCWD, path, O_WRONLY | O_CREAT | O_TRUNC, 0o666)
}

impl Target {
    unsafe fn apply_settings(&self, extra_rules: &[SeccompRule]) -> IOResult<()> {
        if let Some(uid) = self.uid {
            check_os_error(libc::setuid(uid))?;
        }

        if let Some(gid) = self.gid {
            check_os_error(libc::setgid(gid))?;
        }

        if let Some(ref input_path) = self.stdin {
            let input_fd = check_os_error(open_read_fd(input_path.as_ptr()))?;
            let stdin_fd = libc::STDIN_FILENO;
            check_os_error(libc::dup2(input_fd, stdin_fd))?;
        }

        if let Some(ref output_path) = self.stdout {
            let output_fd = check_os_error(open_write_fd(output_path.as_ptr()))?;
            let stdout_fd = libc::STDOUT_FILENO;
            check_os_error(libc::dup2(output_fd, stdout_fd))?;
        }

        if let Some(ref error_path) = self.stderr {
            let error_fd = check_os_error(open_write_fd(error_path.as_ptr()))?;
            let stderr_fd = libc::STDERR_FILENO;
            check_os_error(libc::dup2(error_fd, stderr_fd))?;
        }

        self.limit.apply_rlimit()?;
        self.rule.apply_seccomp(extra_rules)?;
        Ok(())
    }

    fn spawn(&self) -> IOResult<libc::pid_t> {
        let argv = {
            let mut argv: Vec<*const c_char> = Vec::with_capacity(self.args.len() + 2);
            argv.push(self.bin_path.as_ptr());
            argv.extend(self.args.iter().map(|s| s.as_ptr()));
            argv.push(libc::PT_NULL as *const c_char);
            argv
        };

        let envp = {
            if !self.forbid_inherited_env && self.envs.is_empty() {
                None
            } else {
                let mut envp: Vec<*const c_char> = Vec::new();

                if !self.forbid_inherited_env {
                    unsafe {
                        extern "C" {
                            static mut environ: *const *const c_char;
                        }
                        use std::ptr::null;
                        let mut ptr = environ;
                        while ptr != null() && *ptr != null() {
                            envp.push(*ptr);
                            ptr = ptr.offset(1);
                        }
                    }
                }

                envp.extend(self.envs.iter().map(|s| s.as_ptr()));
                envp.push(libc::PT_NULL as *const c_char);
                Some(envp)
            }
        };

        let extra_rules = {
            let execve_rule = {
                if let Action::Allow = self.rule.default_action {
                    if !self.forbid_target_execve {
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

                    if self.forbid_target_execve {
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
        let (rx_fd, tx_fd) = unsafe {
            let mut fds: [c_int; 2] = [0; 2];
            check_os_error(libc::pipe(fds.as_mut_ptr()))?;
            (fds[0], fds[1])
        };

        let ret = unsafe { check_os_error(libc::fork())? };

        if ret == 0 {
            // child process
            unsafe {
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
                    if let Some(envp) = envp {
                        libc::execve(argv[0], argv.as_ptr(), envp.as_ptr());
                    } else {
                        libc::execvp(argv[0], argv.as_ptr());
                    }
                    // child process ends here on success

                    errno = get_errno();
                }

                libc::exit(errno)
            }
        } else {
            // parent process
            unsafe {
                let _ = libc::close(tx_fd);

                let pid = ret;

                // receive errno
                let mut bytes: [u8; 4] = [0; 4];
                let ret = libc::read(rx_fd, bytes.as_mut_ptr() as *mut libc::c_void, bytes.len());
                let _ = libc::close(rx_fd);

                if ret < 0 {
                    let errno = get_errno();
                    let _ = libc::kill(pid, libc::SIGKILL);
                    return Err(IOError::from_raw_os_error(errno)); // error of libc::read
                }

                let errno = i32::from_ne_bytes(bytes);

                if errno == 0 {
                    Ok(pid)
                } else {
                    Err(IOError::from_raw_os_error(errno))
                }
            }
        }
    }

    fn wait(&self, pid: libc::pid_t) -> IOResult<TargetStatus> {
        if let Some(max_real_time) = self.limit.max_real_time {
            thread::Builder::new().spawn(move || unsafe {
                let _ = libc::usleep(max_real_time);
                let _ = libc::kill(pid, libc::SIGKILL);
            })?;
        }

        let mut status = unsafe { std::mem::zeroed::<c_int>() };
        let mut ru = unsafe { std::mem::zeroed::<libc::rusage>() };
        let t0 = SystemTime::now();

        unsafe {
            let ret = libc::wait4(
                pid,
                &mut status as *mut c_int,
                libc::WSTOPPED,
                &mut ru as *mut libc::rusage,
            );
            if ret < 0 {
                let errno = get_errno();
                let _ = libc::kill(pid, libc::SIGKILL);
                return Err(IOError::from_raw_os_error(errno)); // error of libc::wait4
            }
        }

        let real_time = t0.elapsed().unwrap().as_micros() as u64;
        let code;
        let signal;
        unsafe {
            let exited = libc::WIFEXITED(status);
            if exited {
                code = Some(libc::WEXITSTATUS(status));
                signal = None;
            } else {
                code = None;
                signal = Some(libc::WTERMSIG(status));
            }
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
    pub fn run(&self) -> IOResult<TargetStatus> {
        self.wait(self.spawn()?)
    }
}
