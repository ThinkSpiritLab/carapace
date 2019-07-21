use carapace::{Signal, Target};

#[test]
fn test_tle() {
    let target = Target::new("./tests/bin/tle".into());
    let status = target.run().unwrap();
    assert_eq!(status.code, Some(0));
    assert_eq!(status.signal, None);
    assert!((2500_000..3500_000).contains(&status.real_time));
    assert!((2500_000..3500_000).contains(&status.user_time));
}

#[test]
fn test_tle_with_rlimit() {
    let mut target = Target::new("./tests/bin/tle".into());
    target.limit.max_real_time = Some(1500_000);
    target.limit.max_cpu_time = Some(1);

    let status = target.run().unwrap();
    assert_eq!(status.code, None);
    assert_eq!(status.signal, Some(Signal::SIGKILL));
    dbg!(&status);
    assert!((950_000..1050_000).contains(&status.real_time));
    assert!((950_000..1000_000).contains(&status.user_time));
}
