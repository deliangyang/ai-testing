//! Heuristic redaction of outbound prompt text before LLM requests leave the gateway.

use std::sync::LazyLock;

use regex::Regex;

use crate::types::{ChatMessage, ChatRequest};

pub const REDACTED: &str = "****";

#[allow(clippy::expect_used)]
fn static_pattern(source: &'static str) -> Regex {
    Regex::new(source).expect("static redact pattern")
}

static JWT: LazyLock<Regex> =
    LazyLock::new(|| static_pattern(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"));

static AWS_ACCESS_KEY_ID: LazyLock<Regex> = LazyLock::new(|| static_pattern(r"AKIA[0-9A-Z]{16}"));

static ALIYUN_ACCESS_KEY: LazyLock<Regex> =
    LazyLock::new(|| static_pattern(r"LTAI[A-Za-z0-9]{8,}"));

static OPENAI_STYLE_SK: LazyLock<Regex> = LazyLock::new(|| static_pattern(r"sk-[A-Za-z0-9]{16,}"));

static ENV_ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    static_pattern(
        r"(?m)^(?P<key>[A-Za-z_][A-Za-z0-9_]*(?:PASSWORD|SECRET|TOKEN|API_KEY|ACCESS_KEY|PRIVATE_KEY|CREDENTIAL|AUTH)[A-Za-z0-9_]*)\s*=\s*(?P<val>\S+)",
    )
});

static ENV_KNOWN_KEYS: LazyLock<Regex> = LazyLock::new(|| {
    static_pattern(
        r"(?m)^(?P<key>(?:AWS_SECRET_ACCESS_KEY|AWS_ACCESS_KEY_ID|DATABASE_PASSWORD|DB_PASSWORD|OPENAI_API_KEY))\s*=\s*(?P<val>\S+)",
    )
});

static YAML_SECRET_KV: LazyLock<Regex> = LazyLock::new(|| {
    static_pattern(
        r##"(?i)(?P<key>password|secret(?:_key)?|api[_-]?key|access[_-]?key(?:_id)?|auth(?:_token)?|client[_-]?secret|private[_-]?key|bearer|token)\s*:\s*['"]?(?P<val>[^'"\n\s#]+)['"]?"##,
    )
});

static YAML_SK_PAIR: LazyLock<Regex> = LazyLock::new(|| {
    static_pattern(
        r##"(?i)(?P<key>secret[_-]?access[_-]?key|access[_-]?key[_-]?secret)\s*:\s*['"]?(?P<val>[^'"\n\s]+)['"]?"##,
    )
});

/// Replaces suspected credentials in free-form prompt text with [`REDACTED`].
#[must_use]
pub fn redact_text(input: &str) -> String {
    let mut out = input.to_owned();
    for pattern in [
        &*JWT,
        &*AWS_ACCESS_KEY_ID,
        &*ALIYUN_ACCESS_KEY,
        &*OPENAI_STYLE_SK,
    ] {
        out = pattern.replace_all(&out, REDACTED).into_owned();
    }
    out = ENV_ASSIGNMENT
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("{}={}", &caps["key"], REDACTED)
        })
        .into_owned();
    out = ENV_KNOWN_KEYS
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("{}={}", &caps["key"], REDACTED)
        })
        .into_owned();
    out = YAML_SECRET_KV
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("{}: {}", &caps["key"], REDACTED)
        })
        .into_owned();
    out = YAML_SK_PAIR
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            format!("{}: {}", &caps["key"], REDACTED)
        })
        .into_owned();
    out
}

/// Redacts every chat message body before the request is sent or handed to another provider.
#[must_use]
pub fn redact_chat_request(mut request: ChatRequest) -> ChatRequest {
    request.messages = request
        .messages
        .iter()
        .map(|m| ChatMessage {
            role: m.role,
            content: redact_text(&m.content),
        })
        .collect();
    request
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::{ChatMessage, MessageRole};

    const RAW_JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    const RAW_AWS_AK: &str = "AKIAIOSFODNN7EXAMPLE";
    const RAW_SK: &str = "sk-abcdefghijklmnopqrstuvwxyz123456";

    #[test]
    fn redacts_dotenv_style_assignments() {
        let input = "OPENAI_API_KEY=sk-live-from-env\nAPP_NAME=demo\n";
        let out = redact_text(input);
        assert!(out.contains("OPENAI_API_KEY=****"), "{out}");
        assert!(!out.contains("sk-live-from-env"), "{out}");
        assert!(out.contains("APP_NAME=demo"));
    }

    #[test]
    fn redacts_yaml_secret_keys() {
        let input = "spring:\n  datasource:\n    password: hunter2\n    username: app\n";
        let out = redact_text(input);
        assert!(out.contains("password: ****"), "{out}");
        assert!(!out.contains("hunter2"), "{out}");
        assert!(out.contains("username: app"));
    }

    #[test]
    fn redacts_jwt_tokens() {
        let input = format!("Authorization: Bearer {RAW_JWT}");
        let out = redact_text(&input);
        assert!(out.contains(REDACTED), "{out}");
        assert!(!out.contains(RAW_JWT), "{out}");
    }

    #[test]
    fn redacts_ak_sk_style_literals() {
        let input = format!(
            "aws:\n  access_key_id: {RAW_AWS_AK}\n  secret_access_key: wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\n"
        );
        let out = redact_text(&input);
        assert!(!out.contains(RAW_AWS_AK), "{out}");
        assert!(
            out.contains("access_key_id: ****") || out.contains(REDACTED),
            "{out}"
        );
        assert!(out.contains("secret_access_key: ****"), "{out}");
        assert!(!out.contains("wJalrXUtnFEMI"), "{out}");
    }

    #[test]
    fn redacts_sk_prefix_in_prompt() {
        let input = format!("Use key {RAW_SK} for tests");
        let out = redact_text(&input);
        assert!(out.contains(REDACTED), "{out}");
        assert!(!out.contains(RAW_SK), "{out}");
    }

    #[test]
    fn redact_chat_request_maps_all_messages() {
        let req = redact_chat_request(ChatRequest {
            model: "m".into(),
            messages: vec![
                ChatMessage {
                    role: MessageRole::System,
                    content: format!("token: {RAW_JWT}"),
                },
                ChatMessage {
                    role: MessageRole::User,
                    content: format!("key={RAW_SK}"),
                },
            ],
        });
        for m in &req.messages {
            assert!(!m.content.contains(RAW_JWT));
            assert!(!m.content.contains(RAW_SK));
            assert!(m.content.contains(REDACTED));
        }
    }
}
