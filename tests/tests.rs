mod test_hello {
    use carapace::syscallz::{Action, Syscall};
    use carapace::{SeccompRule, Target};

    #[test]
    fn test_hello() {
        let target = Target::new("./tests/bin/hello").unwrap();
        let status = target.run().unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
    }

    #[test]
    fn test_hello_with_seccomp() {
        let mut target = Target::new("./tests/bin/hello").unwrap();
        target.rule.default_action = Some(Action::Allow);
        target.rule.seccomp_rules.push(SeccompRule {
            action: Action::Kill,
            syscall: Syscall::execve,
            comparators: vec![],
        });
        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS)); // `execve` is disabled.
    }
}

mod test_fork {
    use carapace::syscallz::{Action, Syscall};
    use carapace::{SeccompRule, Target};

    #[test]
    fn test_fork_raw() {
        let target = Target::new("./tests/bin/fork").unwrap();
        let status = target.run().unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
    }

    #[test]
    fn test_fork_with_rlimit() {
        let mut target = Target::new("./tests/bin/fork").unwrap();
        target.limit.max_process_number = Some(1);
        let status = target.run().unwrap();
        assert_eq!(status.code, Some(1));
        assert_eq!(status.signal, None);
    }

    #[test]
    fn test_fork_with_seccomp() {
        let mut target = Target::new("./tests/bin/fork").unwrap();
        target.rule.default_action = Some(Action::Allow);
        target.rule.seccomp_rules.push(SeccompRule {
            action: Action::Kill,
            syscall: Syscall::clone,
            comparators: vec![],
        });

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS));
    }

}

mod test_mle {
    use carapace::Target;

    #[test]
    fn test_mle() {
        let target = Target::new("./tests/bin/mle").unwrap();
        let status = target.run().unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
        assert!((32000..35000).contains(&status.memory));
    }

    #[test]
    fn test_mle_with_rlimit() {
        let mut target = Target::new("./tests/bin/mle").unwrap();
        target.limit.max_memory = Some(10000);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSEGV));
        assert!((1000..1500).contains(&status.memory));
    }

}

mod test_real_tle {
    use carapace::Target;

    #[test]
    fn test_real_tle() {
        let mut target = Target::new("./tests/bin/real_tle").unwrap();
        target.limit.max_real_time = Some(1000_000);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGKILL));
        assert!((1000_000..1010_000).contains(&status.real_time));
    }

}

mod test_tle {
    use carapace::Target;

    #[test]
    fn test_tle() {
        let target = Target::new("./tests/bin/tle").unwrap();
        let status = target.run().unwrap();
        assert_eq!(status.code, Some(0));
        assert_eq!(status.signal, None);
        assert!((2500_000..3500_000).contains(&status.real_time));
        assert!((2500_000..3500_000).contains(&status.user_time));
    }

    #[test]
    fn test_tle_with_rlimit() {
        let mut target = Target::new("./tests/bin/tle").unwrap();
        target.limit.max_real_time = Some(1500_000);
        target.limit.max_cpu_time = Some(1);

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGKILL));
        assert!((950_000..1050_000).contains(&status.real_time));
        assert!((950_000..1050_000).contains(&status.user_time));
    }

}

mod test_execvp {
    use carapace::Target;

    #[test]
    fn test_execvp() {
        let mut target = Target::new("./tests/bin/execvp").unwrap();
        target.limit.max_real_time = Some(1000_000);
        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGKILL));
    }

    #[test]
    fn test_execvp_with_seccomp() {
        let mut target = Target::new("./tests/bin/execvp").unwrap();
        target.allow_target_execve = false;

        let status = target.run().unwrap();
        assert_eq!(status.code, None);
        assert_eq!(status.signal, Some(libc::SIGSYS));
    }
}
