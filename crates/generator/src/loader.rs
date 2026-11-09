use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::PromptError;
use crate::template::PromptTemplate;

/// Manifest beside templates (`prompts/manifest.yaml`).
#[derive(Debug, Clone, Deserialize)]
struct ManifestFile {
    #[allow(dead_code)]
    version: u32,
    templates: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestEntry {
    id: String,
    file: String,
}

#[derive(Debug, Clone, Deserialize)]
struct FrontMatter {
    #[serde(default)]
    id: Option<String>,
    prompt_version: String,
}

/// Loaded index of versioned templates under a `prompts/` directory.
#[derive(Debug, Clone)]
pub struct PromptCatalog {
    root: PathBuf,
    templates: BTreeMap<String, ManifestEntry>,
}

impl PromptCatalog {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, PromptError> {
        let root = root.as_ref().to_path_buf();
        let manifest_path = root.join("manifest.yaml");
        if !manifest_path.is_file() {
            return Err(PromptError::ManifestNotFound(manifest_path));
        }
        let raw = fs::read_to_string(&manifest_path).map_err(|source| PromptError::Io {
            path: manifest_path.clone(),
            source,
        })?;
        let manifest: ManifestFile =
            yaml_serde::from_str(&raw).map_err(|e| PromptError::InvalidManifest(e.to_string()))?;
        let mut templates = BTreeMap::new();
        for entry in manifest.templates {
            if templates.contains_key(&entry.id) {
                return Err(PromptError::InvalidManifest(format!(
                    "duplicate template id {}",
                    entry.id
                )));
            }
            templates.insert(entry.id.clone(), entry);
        }
        Ok(Self { root, templates })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn template_ids(&self) -> impl Iterator<Item = &str> {
        self.templates.keys().map(String::as_str)
    }

    pub fn load_template(&self, id: &str) -> Result<PromptTemplate, PromptError> {
        let entry = self
            .templates
            .get(id)
            .ok_or_else(|| PromptError::UnknownTemplate { id: id.to_owned() })?;
        let path = self.root.join(&entry.file);
        if !path.is_file() {
            return Err(PromptError::TemplateFileMissing {
                id: id.to_owned(),
                path,
            });
        }
        let raw = fs::read_to_string(&path).map_err(|source| PromptError::Io {
            path: path.clone(),
            source,
        })?;
        parse_template_file(&path, id, &raw)
    }

    pub fn load_dag_generate_main(&self) -> Result<PromptTemplate, PromptError> {
        self.load_template("dag_generate_main")
    }
}

fn parse_template_file(
    path: &Path,
    manifest_id: &str,
    raw: &str,
) -> Result<PromptTemplate, PromptError> {
    let (front, body) = split_front_matter(raw).ok_or_else(|| PromptError::InvalidFrontMatter {
        path: path.to_path_buf(),
        reason: "expected YAML front matter delimited by ---".into(),
    })?;
    let meta: FrontMatter =
        yaml_serde::from_str(front).map_err(|e| PromptError::InvalidFrontMatter {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
    let id = meta.id.as_deref().unwrap_or(manifest_id);
    if id != manifest_id {
        return Err(PromptError::InvalidFrontMatter {
            path: path.to_path_buf(),
            reason: format!(
                "front matter id {:?} does not match manifest id {manifest_id}",
                meta.id
            ),
        });
    }
    Ok(PromptTemplate {
        id: manifest_id.to_owned(),
        prompt_version: meta.prompt_version,
        body: body.trim_start().to_owned(),
    })
}

fn split_front_matter(raw: &str) -> Option<(&str, &str)> {
    let trimmed = raw.strip_prefix("---")?;
    let rest = trimmed
        .strip_prefix('\n')
        .or_else(|| trimmed.strip_prefix("\r\n"))?;
    let end = rest.find("\n---")?;
    let front = &rest[..end];
    let body = rest[end + 4..]
        .strip_prefix('\n')
        .unwrap_or(&rest[end + 4..]);
    Some((front, body))
}

/// Default `prompts/` directory at the repository root (for tests and local CLI).
pub fn default_prompts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../prompts")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn loads_repo_prompts() {
        let catalog = PromptCatalog::load(default_prompts_dir()).unwrap();
        let tmpl = catalog.load_dag_generate_main().unwrap();
        assert_eq!(tmpl.id, "dag_generate_main");
        assert_eq!(tmpl.prompt_version, "1.0.0");
        assert!(tmpl.body.contains("{openapi_json}"));
    }
}
