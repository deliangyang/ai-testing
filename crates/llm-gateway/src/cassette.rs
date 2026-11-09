use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::provider::{ChatFuture, LlmProvider};
use crate::request_key::chat_request_key;
use crate::types::{ChatRequest, ChatResponse};

/// On-disk cassette (`tests/fixtures/cassettes/<hash>.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CassetteFile {
    pub version: u32,
    pub request: ChatRequest,
    pub response: ChatResponse,
}

/// Deterministic replay from pre-recorded cassettes keyed by canonical request hash.
#[derive(Debug)]
pub struct CassetteProvider {
    dir: PathBuf,
    responses: BTreeMap<String, ChatResponse>,
}

impl CassetteProvider {
    /// Load every `*.json` cassette in `dir`. Filenames must match `chat_request_key(request)`.
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self, LlmError> {
        let dir = dir.as_ref().to_path_buf();
        let mut responses = BTreeMap::new();
        if !dir.is_dir() {
            return Err(LlmError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("cassette directory not found: {}", dir.display()),
            )));
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            let cassette: CassetteFile =
                serde_json::from_str(&text).map_err(|e| LlmError::CassetteInvalid {
                    path: path.display().to_string(),
                    message: e.to_string(),
                })?;
            if cassette.version != 1 {
                return Err(LlmError::CassetteInvalid {
                    path: path.display().to_string(),
                    message: format!("unsupported version {}", cassette.version),
                });
            }
            let key = chat_request_key(&cassette.request)?;
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if file_stem != key {
                return Err(LlmError::CassetteInvalid {
                    path: path.display().to_string(),
                    message: format!("filename stem {file_stem} does not match request hash {key}"),
                });
            }
            responses.insert(key, cassette.response);
        }
        Ok(Self { dir, responses })
    }

    #[must_use]
    pub fn cassette_dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.responses.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.responses.is_empty()
    }

    /// Write a cassette file (for authoring fixtures / optional record mode).
    pub fn write_cassette(
        dir: impl AsRef<Path>,
        request: &ChatRequest,
        response: &ChatResponse,
    ) -> Result<PathBuf, LlmError> {
        let key = chat_request_key(request)?;
        std::fs::create_dir_all(dir.as_ref())?;
        let path = dir.as_ref().join(format!("{key}.json"));
        let file = CassetteFile {
            version: 1,
            request: request.clone(),
            response: response.clone(),
        };
        let json = model::to_canonical_json(&file)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }
}

impl LlmProvider for CassetteProvider {
    fn chat(&self, request: ChatRequest) -> ChatFuture {
        let key = match chat_request_key(&request) {
            Ok(k) => k,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let responses = self.responses.clone();
        Box::pin(async move {
            responses
                .get(&key)
                .cloned()
                .ok_or(LlmError::CassetteMiss { key })
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod fixture_author {
    use super::*;
    use crate::types::{ChatMessage, MessageRole, TokenUsage};

    #[test]
    #[ignore = "authoring helper"]
    fn write_sample_cassette() {
        let request = ChatRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![
                ChatMessage {
                    role: MessageRole::System,
                    content: "You are terse.".into(),
                },
                ChatMessage {
                    role: MessageRole::User,
                    content: "Say pong.".into(),
                },
            ],
        };
        let response = ChatResponse {
            content: "pong".into(),
            model: "gpt-4o-mini".into(),
            usage: TokenUsage {
                prompt_tokens: 8,
                completion_tokens: 4,
                total_tokens: 12,
            },
            finish_reason: Some("stop".into()),
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cassettes");
        let path = CassetteProvider::write_cassette(&dir, &request, &response).unwrap();
        eprintln!("wrote {}", path.display());
    }
}
