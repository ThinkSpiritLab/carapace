use carapace::syscallz::{Action, Syscall};
use carapace::{SeccompRule, Signal, Target};

#[test]
fn test_fork() {
    let target = Target::new("./tests/bin/fork".into());
    let status = target.run().unwrap();
    assert_eq!(status.code, Some(0));
    assert_eq!(status.signal, None);
}

#[test]
fn test_fork_with_rlimit() {
    let mut target = Target::new("./tests/bin/fork".into());
    target.limit.max_process_number = Some(1);
    let status = target.run().unwrap();
    assert_eq!(status.code, Some(1));
    assert_eq!(status.signal, None);
}

#[test]
fn test_fork_with_seccomp() {
    let mut target = Target::new("./tests/bin/fork".into());
    target.rule.default_action = Some(Action::Allow);
    target.rule.seccomp_rules.push(SeccompRule {
        action: Action::Kill,
        syscall: Syscall::clone,
        comparators: vec![],
    });

    let status = target.run().unwrap();
    assert_eq!(status.code, None);
    assert_eq!(status.signal, Some(Signal::SIGSYS));
}
