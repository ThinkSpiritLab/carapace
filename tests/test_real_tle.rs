use carapace::{Signal, Target};

#[test]
fn test_real_tle() {
    let mut target = Target::new("./tests/bin/real_tle".into());
    target.limit.max_real_time = Some(1000_000);

    let status = target.run().unwrap();
    assert_eq!(status.code, None);
    assert_eq!(status.signal, Some(Signal::SIGKILL));
    assert!((1000_000..1005_000).contains(&status.real_time));
}
