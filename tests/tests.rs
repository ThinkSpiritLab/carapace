#![forbid(unsafe_code)]

mod test_hello {
    use carapace::syscallz::{Action, Syscall};
    use carapace::Target;

    const BIN: &str = "./tests/bin/hello";

    #[test]
    fn test_hello_raw() {
        let mut target = Target::new(BIN).unwrap();
        target.stdout("/dev/null").unwrap();

        let status = target.run().unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
    }

    #[test]
    fn test_hello_with_seccomp() {
        let mut target = Target::new(BIN).unwrap();
        target.rule.add_action(Action::Kill, Syscall::execve);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS)); // `execve` is disabled.
    }

    #[test]
    fn test_hello_with_c_cpp() {
        use super::test_all_c_cpp::test_one;
        let status = test_one(BIN, |_| {});
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
    }
}

mod test_fork {
    use carapace::syscallz::{Action, Syscall};
    use carapace::Target;

    const BIN: &str = "./tests/bin/fork";

    #[test]
    fn test_fork_raw() {
        let mut target = Target::new(BIN).unwrap();
        target.stdout("/dev/null").unwrap();

        let status = target.run().unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
    }

    #[test]
    fn test_fork_with_rlimit() {
        let mut target = Target::new(BIN).unwrap();
        target.limit.max_process_number = Some(1);
        target.stdout("/dev/null").unwrap();

        let status = target.run().unwrap();
        assert_eq!(status.code, Some(1));
        assert_eq!(status.signal, None);
    }

    #[test]
    fn test_fork_with_seccomp() {
        let mut target = Target::new(BIN).unwrap();
        target.rule.add_action(Action::Kill, Syscall::clone);
        target.stdout("/dev/null").unwrap();

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS));
    }

    #[test]
    fn test_fork_with_c_cpp() {
        use super::test_all_c_cpp::test_one;
        let status = test_one(BIN, |_| {});
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS));
    }

}

mod test_mle {
    use carapace::Target;

    const BIN: &str = "./tests/bin/mle";

    #[test]
    fn test_mle_raw() {
        let target = Target::new(BIN).unwrap();

        let status = target.run().unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
        assert!((32000..35000).contains(&status.memory));
    }

    #[test]
    fn test_mle_with_rlimit() {
        let mut target = Target::new(BIN).unwrap();
        target.limit.max_memory = Some(10000);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSEGV));
        assert!((1000..3000).contains(&status.memory));
    }

    #[test]
    fn test_mle_with_c_cpp() {
        use super::test_all_c_cpp::test_one;
        let status = test_one(BIN, |target: &mut Target| {
            target.limit.max_memory = Some(10000)
        });
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSEGV));
        assert!((1000..3000).contains(&status.memory));
    }

}

mod test_real_tle {
    use carapace::Target;

    const BIN: &str = "./tests/bin/real_tle";

    #[test]
    fn test_real_tle_raw() {
        let mut target = Target::new(BIN).unwrap();
        target.limit.max_real_time = Some(1000_000);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGKILL));
        assert!((1000_000..1050_000).contains(&status.real_time));
    }

    #[test]
    fn test_real_tle_with_c_cpp() {
        use super::test_all_c_cpp::test_one;
        let status = test_one(BIN, |_| {});
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS)); // `sleep` is disabled
    }
}

mod test_tle {
    use carapace::Target;

    const BIN: &str = "./tests/bin/tle";

    #[test]
    fn test_tle_raw() {
        let target = Target::new(BIN).unwrap();

        let status = target.run().unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
        assert!((2000_000..3500_000).contains(&status.real_time));
        assert!((2000_000..3500_000).contains(&status.user_time));
    }

    #[test]
    fn test_tle_with_rlimit() {
        let mut target = Target::new(BIN).unwrap();
        target.limit.max_real_time = Some(1500_000);
        target.limit.max_cpu_time = Some(1);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGKILL));
        assert!((500_000..1500_000).contains(&status.real_time));
        assert!((500_000..1500_000).contains(&status.user_time));
    }

    #[test]
    fn test_tle_with_c_cpp() {
        use super::test_all_c_cpp::test_one;
        let status = test_one(BIN, |_| {});
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGKILL));
        assert!((500_000..1500_000).contains(&status.real_time));
        assert!((500_000..1500_000).contains(&status.user_time));
    }

}

mod test_execvp {
    use carapace::Target;

    const BIN: &str = "./tests/bin/execvp";

    #[test]
    fn test_execvp_raw() {
        let mut target = Target::new(BIN).unwrap();
        target.limit.max_real_time = Some(1000_000);
        target.stdout("/dev/null").unwrap();

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGKILL));
    }

    #[test]
    fn test_execvp_with_seccomp() {
        let mut target = Target::new(BIN).unwrap();
        target.allow_target_execve = false;
        target.stdout("/dev/null").unwrap();

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS));
    }

    #[test]
    fn test_execvp_with_c_cpp() {
        use super::test_all_c_cpp::test_one;
        let status = test_one(BIN, |_| {});
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS));
    }
}

mod test_forkbomb {
    use carapace::syscallz::{Action, Syscall};
    use carapace::Target;

    const BIN: &str = "./tests/bin/forkbomb";

    #[test]
    fn test_forkbomb_with_rlimit() {
        let mut target = Target::new(BIN).unwrap();
        target.limit.max_real_time = Some(1000_000);
        target.limit.max_process_number = Some(1);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGKILL));
    }

    #[test]
    fn test_forkbomb_with_seccomp() {
        let mut target = Target::new(BIN).unwrap();
        target.limit.max_real_time = Some(1000_000);
        target.rule.add_action(Action::Kill, Syscall::clone);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS));
    }

    #[test]
    fn test_forkbomb_with_c_cpp() {
        use super::test_all_c_cpp::test_one;
        let status = test_one(BIN, |_| {});
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS));
    }
}

mod test_bigfile {
    use carapace::Target;

    const BIN: &str = "gcc";
    const ARG: &str = "./tests/case/bigfile.c";

    #[test]
    fn test_bigfile_with_rlimit() {
        let mut target = Target::new(BIN).unwrap();
        target.add_arg(ARG).unwrap();
        target.limit.max_memory = Some(256 * 1024 * 1024);

        let status = target.run().unwrap();
        assert_eq!(status.code, Some(1));
        assert_eq!(status.signal, None);
    }
}

mod test_all_c_cpp {
    use carapace::built_in::{c_cpp_rules, getPATH};
    use carapace::syscallz::{Action, Syscall};
    use carapace::{Target, TargetStatus};

    pub(super) fn test_one(
        bin_path: &str,
        configure: impl FnOnce(&mut Target) -> (),
    ) -> TargetStatus {
        let mut target = Target::new(bin_path).unwrap();

        target.envs.push(getPATH().unwrap());

        target.stdin("/dev/null").unwrap();
        target.stdout("/dev/null").unwrap();
        target.stderr("/dev/null").unwrap();

        target.limit.max_real_time = Some(1000_000);
        target.limit.max_memory = Some(256 * 1024 * 1024);
        target.limit.max_output_size = Some(32 * 1024 * 1024);
        target.limit.max_cpu_time = Some(1);

        target.rule = c_cpp_rules();

        target.rule.add_action(Action::Allow, Syscall::ioctl); // for /dev/null

        target.allow_inherited_env = false;
        target.allow_target_execve = false;

        configure(&mut target);
        let status = target.run().unwrap();
        dbg!((bin_path, &status));
        return status;
    }
}
