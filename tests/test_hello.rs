use carapace::Target;

#[test]
fn test_hello() {
    let target = Target::new("./tests/bin/hello".into());
    let status = target.run().unwrap();
    assert_eq!(status.code, Some(0));
    assert_eq!(status.signal, None);
}
