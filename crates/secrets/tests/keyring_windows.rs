#![cfg(windows)]
#![allow(clippy::unwrap_used)]

use secrets::{delete_keychain_secret, read_keychain_secret, write_keychain_secret};

const SERVICE: &str = "ai-testing";
const ACCOUNT: &str = "p2-01-test";

#[test]
fn windows_credential_manager_round_trip() {
    let _ = delete_keychain_secret(SERVICE, ACCOUNT);
    let secret = "p2-01-round-trip-secret";
    write_keychain_secret(SERVICE, ACCOUNT, secret).unwrap();
    let loaded = read_keychain_secret(SERVICE, ACCOUNT).unwrap();
    assert_eq!(loaded, secret);
    delete_keychain_secret(SERVICE, ACCOUNT).unwrap();
}
