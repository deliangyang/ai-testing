use model::to_canonical_json;
use sha2::{Digest, Sha256};

use crate::types::ChatRequest;

use crate::error::LlmError;

/// Stable SHA-256 hex of the canonical JSON request (model + messages).
pub fn chat_request_key(request: &ChatRequest) -> Result<String, LlmError> {
    let canonical = to_canonical_json(request)?;
    Ok(sha256_hex(canonical.as_bytes()))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::{ChatMessage, MessageRole};

    #[test]
    fn same_request_same_key() {
        let req = ChatRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "ping".into(),
            }],
        };
        let a = chat_request_key(&req).unwrap();
        let b = chat_request_key(&req).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn different_messages_different_key() {
        let a = ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "a".into(),
            }],
        };
        let b = ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: "b".into(),
            }],
        };
        assert_ne!(chat_request_key(&a).unwrap(), chat_request_key(&b).unwrap());
    }
}
