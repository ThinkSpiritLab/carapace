use carapace::{Signal, Target};

#[test]
fn test_mle() {
    let target = Target::new("./tests/bin/mle".into());
    let status = target.run().unwrap();
    assert_eq!(status.code, Some(0));
    assert_eq!(status.signal, None);
    assert!((32000..35000).contains(&status.memory));
}

#[test]
fn test_mle_with_rlimit() {
    let mut target = Target::new("./tests/bin/mle".into());
    target.limit.max_memory = Some(10000);

    let status = target.run().unwrap();
    assert_eq!(status.code, None);
    assert_eq!(status.signal, Some(Signal::SIGSEGV));
    assert!((1000..1500).contains(&status.memory));
}
