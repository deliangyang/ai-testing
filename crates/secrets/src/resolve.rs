use std::env;

use crate::error::{Result, SecretsError};
use crate::reference::{parse_credential_ref, CredentialRef};

pub fn resolve_credential_ref(reference: &str) -> Result<String> {
    resolve(&parse_credential_ref(reference)?)
}

pub fn resolve(cred: &CredentialRef) -> Result<String> {
    match cred {
        CredentialRef::Keychain { service, account } => {
            crate::keychain::read_keychain_secret(service, account)
        }
        CredentialRef::Env { name } => {
            env::var(name).map_err(|_| SecretsError::EnvNotSet(name.clone()))
        }
    }
}
