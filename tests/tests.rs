mod test_hello {
    use carapace::syscallz::{Action, Syscall};
    use carapace::Target;

    const BIN: &str = "./tests/bin/hello";

    #[test]
    fn test_hello_raw() {
        let target = Target::new(BIN).unwrap();

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

        let status = target.run().unwrap();
        assert_eq!(status.code, Some(1));
        assert_eq!(status.signal, None);
    }

    #[test]
    fn test_fork_with_seccomp() {
        let mut target = Target::new(BIN).unwrap();
        target.rule.add_action(Action::Kill, Syscall::clone);

        let status = target.run().unwrap();
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
        assert!((1000..1500).contains(&status.memory));
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
        assert!((1000_000..1010_000).contains(&status.real_time));
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
        assert!((2500_000..3500_000).contains(&status.real_time));
        assert!((2500_000..3500_000).contains(&status.user_time));
    }

    #[test]
    fn test_tle_with_rlimit() {
        let mut target = Target::new(BIN).unwrap();
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

        let status = target.run().unwrap();
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
