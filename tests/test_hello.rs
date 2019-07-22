use carapace::syscallz::Action;
use carapace::{Signal, Target};

#[test]
fn test_hello() {
    let target = Target::new("./tests/bin/hello".into());
    let status = target.run().unwrap();
    assert_eq!(status.code, Some(0));
    assert_eq!(status.signal, None);
}

#[test]
fn test_hello_with_seccomp_default_kill() {
    let mut target = Target::new("./tests/bin/hello".into());
    target.rule.default_action = Some(Action::Kill);
    let status = target.run().unwrap();
    assert_eq!(status.code, None);
    assert_eq!(status.signal, Some(Signal::SIGSYS)); // because `execve` is disabled.
}
