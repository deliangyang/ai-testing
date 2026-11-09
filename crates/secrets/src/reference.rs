//! Parsing and validation for stored credential reference strings.

use model::template::{Part, Template, VarRef};

use crate::error::{Result, SecretsError};

const KEYCHAIN_PREFIX: &str = "keychain://";

/// A parsed credential reference (no secret material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialRef {
    Keychain { service: String, account: String },
    Env { name: String },
}

/// Accepts `None`, rejects plaintext and malformed references.
pub fn validate_credential_ref(value: Option<&str>) -> Result<()> {
    match value {
        None => Ok(()),
        Some(s) if s.trim().is_empty() => Err(SecretsError::PlaintextCredential),
        Some(s) => {
            parse_credential_ref(s)?;
            Ok(())
        }
    }
}

pub fn parse_credential_ref(s: &str) -> Result<CredentialRef> {
    if let Some(rest) = s.strip_prefix(KEYCHAIN_PREFIX) {
        return parse_keychain(rest);
    }
    parse_env_whole_string(s)
}

fn parse_keychain(rest: &str) -> Result<CredentialRef> {
    let Some((service, account)) = rest.split_once('/') else {
        return Err(SecretsError::InvalidReference(
            "keychain URL must be keychain://<service>/<account>".into(),
        ));
    };
    if service.is_empty() || account.is_empty() {
        return Err(SecretsError::InvalidReference(
            "keychain service and account must be non-empty".into(),
        ));
    }
    if service.contains('/') || account.contains('/') {
        return Err(SecretsError::InvalidReference(
            "keychain service and account must not contain '/'".into(),
        ));
    }
    validate_keychain_segment(service, "service")?;
    validate_keychain_segment(account, "account")?;
    Ok(CredentialRef::Keychain {
        service: service.to_owned(),
        account: account.to_owned(),
    })
}

fn validate_keychain_segment(segment: &str, label: &str) -> Result<()> {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return Err(SecretsError::InvalidReference(format!(
            "keychain {label} must be non-empty"
        )));
    };
    if !(first.is_ascii_alphanumeric() || first == '_') {
        return Err(SecretsError::InvalidReference(format!(
            "keychain {label} must start with a letter, digit, or '_'"
        )));
    }
    for c in chars {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            continue;
        }
        return Err(SecretsError::InvalidReference(format!(
            "invalid character in keychain {label}"
        )));
    }
    Ok(())
}

/// The stored value must be exactly one `${env:NAME}` reference (same env-name rules as DAG templates).
fn parse_env_whole_string(s: &str) -> Result<CredentialRef> {
    let template = Template::parse(s).map_err(|e| SecretsError::InvalidReference(e.to_string()))?;
    match template.parts.as_slice() {
        [Part::Ref(VarRef::Env { name })] => Ok(CredentialRef::Env { name: name.clone() }),
        [Part::Ref(VarRef::Auth { .. })] | [Part::Ref(VarRef::Node { .. })] => {
            Err(SecretsError::PlaintextCredential)
        }
        _ => Err(SecretsError::PlaintextCredential),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_keychain_refs() {
        let cases = [
            "keychain://ai-testing/p2-01-test",
            "keychain://my.service/my_account",
            "keychain://SVC/account1",
        ];
        for input in cases {
            assert!(parse_credential_ref(input).is_ok(), "expected ok: {input}");
        }
    }

    #[test]
    fn accepts_valid_env_refs() {
        let cases = ["${env:API_KEY}", "${env:AITEST_TEST_SECRET}", "${env:_X}"];
        for input in cases {
            let cred = parse_credential_ref(input).unwrap();
            assert!(matches!(cred, CredentialRef::Env { .. }), "input: {input}");
        }
    }

    #[test]
    fn rejects_plaintext_and_malformed_refs() {
        let cases = [
            "Bearer sk-live-abc123",
            "super-secret-password",
            "keychain://only-service",
            "keychain:///account",
            "keychain://service/",
            "keychain://",
            "${env:}",
            "${env:A-B}",
            "prefix ${env:VAR}",
            "${env:VAR} suffix",
            "${auth.token}",
        ];
        for input in cases {
            assert!(
                parse_credential_ref(input).is_err(),
                "expected err: {input}"
            );
        }
    }

    #[test]
    fn validate_optional_auth_ref() {
        assert!(validate_credential_ref(None).is_ok());
        assert!(validate_credential_ref(Some("${env:TOK}")).is_ok());
        assert!(validate_credential_ref(Some("plaintext")).is_err());
        assert!(validate_credential_ref(Some("")).is_err());
    }
}
