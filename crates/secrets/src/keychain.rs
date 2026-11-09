use keyring::Entry;

use crate::error::{Result, SecretsError};

fn entry(service: &str, account: &str) -> Result<Entry> {
    Entry::new(service, account).map_err(|e| SecretsError::Keychain(e.to_string()))
}

/// Stores `secret` in the OS credential store under `service` / `account`.
pub fn write_keychain_secret(service: &str, account: &str, secret: &str) -> Result<()> {
    entry(service, account)?
        .set_password(secret)
        .map_err(|e| SecretsError::Keychain(e.to_string()))
}

pub fn read_keychain_secret(service: &str, account: &str) -> Result<String> {
    entry(service, account)?
        .get_password()
        .map_err(|e| SecretsError::Keychain(e.to_string()))
}

/// Removes a credential (test cleanup).
pub fn delete_keychain_secret(service: &str, account: &str) -> Result<()> {
    entry(service, account)?
        .delete_credential()
        .map_err(|e| SecretsError::Keychain(e.to_string()))
}
