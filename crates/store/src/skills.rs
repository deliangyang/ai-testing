//! Versioned Skill packages: `pending` on save, one `active` per id, the rest `archived`.

use std::fmt;
use std::str::FromStr;

use model::nodes::SkillRef;
use model::{to_canonical_json, Dag};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use skills::{BundledAssertion, SkillIoContract, SkillManifest, SkillPackage};

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkillStatus {
    Pending,
    Active,
    Archived,
}

impl SkillStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SkillStatus::Pending => "pending",
            SkillStatus::Active => "active",
            SkillStatus::Archived => "archived",
        }
    }
}

impl fmt::Display for SkillStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SkillStatus {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(SkillStatus::Pending),
            "active" => Ok(SkillStatus::Active),
            "archived" => Ok(SkillStatus::Archived),
            other => Err(StoreError::Corrupt(format!(
                "unknown Skill status '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillVersionInfo {
    pub id: String,
    /// Activation revision (monotonic per `id`); not the manifest semver.
    pub version: u32,
    pub name: String,
    /// Manifest semver string (e.g. `1.0.0`).
    pub manifest_version: String,
    pub status: SkillStatus,
    pub tsx_plugin_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillRecord {
    pub info: SkillVersionInfo,
    pub package: SkillPackage,
}

/// Body stored in `content_json`; contracts live in dedicated columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillStoredContent {
    manifest: SkillManifest,
    dag: Dag,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    assertions: Vec<BundledAssertion>,
}

pub struct SkillRepository<'a> {
    conn: &'a Connection,
}

const INFO_COLUMNS: &str = "id, version, name, status, content_json, input_contract_json, output_contract_json, tsx_plugin_ref";

impl<'a> SkillRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Stores `package` as the next activation revision of `manifest.id`, in `pending` state.
    pub fn save(&self, package: &SkillPackage) -> Result<SkillRecord> {
        let (content_json, input_json, output_json) = encode_split(package)?;

        let tx = Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?;
        let id = &package.manifest.id;
        let latest: Option<u32> =
            tx.query_row("SELECT max(version) FROM skills WHERE id = ?1", [id], |r| {
                r.get(0)
            })?;
        let version = latest.unwrap_or(0) + 1;
        tx.execute(
            "INSERT INTO skills (id, version, name, content_json, input_contract_json, output_contract_json, tsx_plugin_ref, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 'pending')",
            params![
                id,
                version,
                package.manifest.name,
                content_json,
                input_json,
                output_json,
            ],
        )?;
        tx.commit()?;

        Ok(SkillRecord {
            info: SkillVersionInfo {
                id: id.clone(),
                version,
                name: package.manifest.name.clone(),
                manifest_version: package.manifest.version.clone(),
                status: SkillStatus::Pending,
                tsx_plugin_ref: None,
            },
            package: package.clone(),
        })
    }

    /// Makes `version` the active revision and archives the previous active one.
    pub fn activate(&self, id: &str, version: u32, expected_active: Option<u32>) -> Result<()> {
        let tx = Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?;
        let exists: Option<String> = tx
            .query_row(
                "SELECT status FROM skills WHERE id = ?1 AND version = ?2",
                params![id, version],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StoreError::NotFound(format!(
                "Skill '{id}' version {version}"
            )));
        }
        let current = active_version(&tx, id)?;
        if current != expected_active {
            return Err(StoreError::Conflict(format!(
                "Skill '{id}' active version is {}, expected {}",
                show(current),
                show(expected_active)
            )));
        }
        if current == Some(version) {
            return Ok(());
        }
        tx.execute(
            "UPDATE skills SET status = 'archived' WHERE id = ?1 AND status = 'active'",
            [id],
        )?;
        tx.execute(
            "UPDATE skills SET status = 'active' WHERE id = ?1 AND version = ?2",
            params![id, version],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: &str, version: u32) -> Result<Option<SkillRecord>> {
        self.conn
            .query_row(
                &format!("SELECT {INFO_COLUMNS} FROM skills WHERE id = ?1 AND version = ?2"),
                params![id, version],
                raw_record,
            )
            .optional()?
            .map(RawRecord::decode)
            .transpose()
    }

    pub fn active(&self, id: &str) -> Result<Option<SkillRecord>> {
        self.conn
            .query_row(
                &format!("SELECT {INFO_COLUMNS} FROM skills WHERE id = ?1 AND status = 'active'"),
                [id],
                raw_record,
            )
            .optional()?
            .map(RawRecord::decode)
            .transpose()
    }

    /// All activation revisions of `id`, oldest first.
    pub fn versions(&self, id: &str) -> Result<Vec<SkillVersionInfo>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {INFO_COLUMNS} FROM skills WHERE id = ?1 ORDER BY version"
        ))?;
        let rows = stmt.query_map([id], raw_row_for_info)?;
        rows.map(|r| r.map_err(StoreError::from).and_then(decode_info_from_row))
            .collect()
    }

    /// Every revision of every Skill, ordered by id then version.
    pub fn list(&self) -> Result<Vec<SkillVersionInfo>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {INFO_COLUMNS} FROM skills ORDER BY id, version"
        ))?;
        let rows = stmt.query_map([], raw_row_for_info)?;
        rows.map(|r| r.map_err(StoreError::from).and_then(decode_info_from_row))
            .collect()
    }

    pub fn active_version(&self, id: &str) -> Result<Option<u32>> {
        active_version(self.conn, id)
    }

    /// Resolves a subworkflow Skill reference.
    ///
    /// Without `version_req`, returns the active revision. With a constraint, picks the highest
    /// matching manifest semver among revisions that have been activated (`active` or `archived`);
    /// `pending` never runs.
    pub fn resolve(&self, skill: &SkillRef) -> Result<Option<SkillRecord>> {
        let Some(version_req) = skill.version_req.as_deref() else {
            return self.active(&skill.id);
        };
        let req = VersionReq::parse(version_req)
            .map_err(|e| StoreError::Corrupt(format!("invalid skill version_req: {e}")))?;
        let mut best: Option<(Version, u32)> = None;
        for info in self.versions(&skill.id)? {
            if info.status == SkillStatus::Pending {
                continue;
            }
            let Ok(manifest) = Version::parse(&info.manifest_version) else {
                return Err(StoreError::Corrupt(format!(
                    "Skill '{}' revision {} has invalid manifest version '{}'",
                    skill.id, info.version, info.manifest_version
                )));
            };
            if !req.matches(&manifest) {
                continue;
            }
            let replace = match &best {
                None => true,
                Some((best_v, best_rev)) => {
                    manifest > *best_v || (manifest == *best_v && info.version > *best_rev)
                }
            };
            if replace {
                best = Some((manifest, info.version));
            }
        }
        match best {
            None => Ok(None),
            Some((_, rev)) => self.get(&skill.id, rev),
        }
    }
}

fn encode_split(package: &SkillPackage) -> Result<(String, String, String)> {
    let mut dag = package.dag.clone();
    dag.version = None;
    let body = SkillStoredContent {
        manifest: package.manifest.clone(),
        dag,
        assertions: package.assertions.clone(),
    };
    let content_json = to_canonical_json(&body).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let input_json =
        to_canonical_json(&package.inputs).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let output_json =
        to_canonical_json(&package.outputs).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    Ok((content_json, input_json, output_json))
}

fn decode_package(content_json: &str, input_json: &str, output_json: &str) -> Result<SkillPackage> {
    let body: SkillStoredContent = serde_json::from_str(content_json)
        .map_err(|e| StoreError::Corrupt(format!("skill content_json: {e}")))?;
    let inputs: SkillIoContract = serde_json::from_str(input_json)
        .map_err(|e| StoreError::Corrupt(format!("skill input_contract_json: {e}")))?;
    let outputs: SkillIoContract = serde_json::from_str(output_json)
        .map_err(|e| StoreError::Corrupt(format!("skill output_contract_json: {e}")))?;
    Ok(SkillPackage {
        manifest: body.manifest,
        inputs,
        outputs,
        dag: body.dag,
        assertions: body.assertions,
    })
}

fn manifest_version_from_content(content_json: &str) -> Result<String> {
    let v: serde_json::Value = serde_json::from_str(content_json)
        .map_err(|e| StoreError::Corrupt(format!("skill content_json: {e}")))?;
    v.get("manifest")
        .and_then(|m| m.get("version"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| StoreError::Corrupt("skill content_json missing manifest.version".into()))
}

fn active_version(conn: &Connection, id: &str) -> Result<Option<u32>> {
    Ok(conn
        .query_row(
            "SELECT version FROM skills WHERE id = ?1 AND status = 'active'",
            [id],
            |r| r.get(0),
        )
        .optional()?)
}

fn show(version: Option<u32>) -> String {
    version.map_or_else(|| "none".to_owned(), |v| v.to_string())
}

struct RawRecord {
    id: String,
    version: u32,
    name: String,
    status: String,
    content_json: String,
    input_json: String,
    output_json: String,
    tsx_plugin_ref: Option<String>,
}

impl RawRecord {
    fn decode(self) -> Result<SkillRecord> {
        let manifest_version = manifest_version_from_content(&self.content_json)?;
        let package = decode_package(&self.content_json, &self.input_json, &self.output_json)?;
        Ok(SkillRecord {
            info: SkillVersionInfo {
                id: self.id,
                version: self.version,
                name: self.name,
                manifest_version,
                status: self.status.parse()?,
                tsx_plugin_ref: self.tsx_plugin_ref,
            },
            package,
        })
    }
}

fn raw_record(r: &Row<'_>) -> rusqlite::Result<RawRecord> {
    Ok(RawRecord {
        id: r.get(0)?,
        version: r.get(1)?,
        name: r.get(2)?,
        status: r.get(3)?,
        content_json: r.get(4)?,
        input_json: r.get(5)?,
        output_json: r.get(6)?,
        tsx_plugin_ref: r.get(7)?,
    })
}

fn raw_row_for_info(r: &Row<'_>) -> rusqlite::Result<RawRecord> {
    raw_record(r)
}

fn decode_info_from_row(raw: RawRecord) -> Result<SkillVersionInfo> {
    let manifest_version = manifest_version_from_content(&raw.content_json)?;
    Ok(SkillVersionInfo {
        id: raw.id,
        version: raw.version,
        name: raw.name,
        manifest_version,
        status: raw.status.parse()?,
        tsx_plugin_ref: raw.tsx_plugin_ref,
    })
}
