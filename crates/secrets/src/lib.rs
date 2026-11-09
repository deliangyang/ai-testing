//! Credential references (`keychain://service/account`, `${env:VAR}`): parse, validate, resolve.

mod error;
mod keychain;
mod reference;
mod resolve;

pub use error::SecretsError;
pub use keychain::{delete_keychain_secret, read_keychain_secret, write_keychain_secret};
pub use reference::{parse_credential_ref, validate_credential_ref, CredentialRef};
pub use resolve::resolve_credential_ref;
