#[test]
#[ignore = "touches the operating-system credential manager"]
fn system_keyring_round_trip() {
    let account = format!("test-{}", std::process::id());
    let entry = keyring::Entry::new("agent-demo.test", &account).unwrap();
    entry.set_password("sentinel-not-a-real-key").unwrap();
    assert_eq!(entry.get_password().unwrap(), "sentinel-not-a-real-key");
    entry.delete_credential().unwrap();
    assert!(matches!(entry.get_password(), Err(keyring::Error::NoEntry)));
}
