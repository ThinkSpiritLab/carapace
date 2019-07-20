use carapace::Target;

#[test]
fn test_mle() {
    let target = Target::new("./tests/bin/mle".into());
    let status = target.run().unwrap();
    assert_eq!(status.code, Some(0));
    assert_eq!(status.signal, None);
    assert!((32000..35000).contains(&status.memory));
}
