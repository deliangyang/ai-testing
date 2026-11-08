//! Embedded, forward-only SQL migrations.
//!
//! Files live in `crates/store/migrations/` as `V###__name.sql` and are compiled in through
//! [`MIGRATIONS`]. Applied migrations are recorded in `schema_migrations` with a SHA-256 of
//! their SQL; an applied file must never change; add a new migration instead.

use std::collections::BTreeMap;

use common::clock::Clock;
use rusqlite::{params, Connection, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::error::{Result, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// File name, `V###__name.sql`.
    pub file: &'static str,
    pub sql: &'static str,
}

macro_rules! migration {
    ($file:literal) => {
        Migration {
            file: $file,
            sql: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/", $file)),
        }
    };
}

/// Every migration of this build, in version order.
pub const MIGRATIONS: &[Migration] = &[
    migration!("V001__core.sql"),
    migration!("V002__node_execution_guards.sql"),
    migration!("V003__ui_layout.sql"),
];

const VERSION_TABLE: &str = "CREATE TABLE IF NOT EXISTS schema_migrations (
    version    INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    checksum   TEXT NOT NULL,
    applied_at TEXT NOT NULL
) STRICT";

impl Migration {
    /// `(version, name)` parsed from the file name.
    pub fn parse_file_name(&self) -> Result<(u32, &'static str)> {
        let invalid = |reason: &str| StoreError::InvalidMigration {
            file: self.file.to_owned(),
            reason: reason.to_owned(),
        };
        let stem = self
            .file
            .strip_suffix(".sql")
            .ok_or_else(|| invalid("must end with '.sql'"))?;
        let rest = stem
            .strip_prefix('V')
            .ok_or_else(|| invalid("must start with 'V'"))?;
        let (digits, name) = rest
            .split_once("__")
            .ok_or_else(|| invalid("must look like V###__name.sql"))?;
        if digits.len() < 3 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("version must be at least three digits"));
        }
        let version: u32 = digits
            .parse()
            .map_err(|_| invalid("version is too large"))?;
        if version == 0 {
            return Err(invalid("versions start at 1"));
        }
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(invalid("name must be lowercase snake_case"));
        }
        Ok((version, name))
    }

    pub fn checksum(&self) -> String {
        Sha256::digest(self.sql.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: u32,
    pub name: String,
    pub checksum: String,
    pub applied_at: String,
}

/// Brings the database up to the last of `migrations` inside one immediate transaction, so
/// concurrent starters serialize and a failing migration leaves nothing behind.
///
/// Returns the versions applied by this call.
pub fn migrate(
    conn: &mut Connection,
    migrations: &[Migration],
    clock: &dyn Clock,
) -> Result<Vec<u32>> {
    let mut known = BTreeMap::new();
    let mut last = 0;
    for m in migrations {
        let (version, name) = m.parse_file_name()?;
        if version <= last {
            return Err(StoreError::InvalidMigration {
                file: m.file.to_owned(),
                reason: format!("versions must be strictly increasing, follows V{last:03}"),
            });
        }
        last = version;
        known.insert(version, (name, m));
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(VERSION_TABLE, [])?;
    let applied = applied_in(&tx)?;
    let max_applied = applied.last().map_or(0, |a| a.version);
    for a in &applied {
        let Some((name, m)) = known.get(&a.version) else {
            return Err(StoreError::UnknownMigration { version: a.version });
        };
        if a.checksum != m.checksum() {
            return Err(StoreError::ChecksumMismatch {
                version: a.version,
                name: (*name).to_owned(),
            });
        }
    }

    let mut newly = Vec::new();
    for (&version, (name, m)) in &known {
        if applied.iter().any(|a| a.version == version) {
            continue;
        }
        if version < max_applied {
            return Err(StoreError::OutOfOrder {
                version,
                applied: max_applied,
            });
        }
        tx.execute_batch(m.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![version, name, m.checksum(), clock.now().to_string()],
        )?;
        newly.push(version);
    }
    tx.commit()?;
    Ok(newly)
}

pub fn applied(conn: &Connection) -> Result<Vec<AppliedMigration>> {
    conn.execute(VERSION_TABLE, [])?;
    applied_in(conn)
}

fn applied_in(conn: &Connection) -> Result<Vec<AppliedMigration>> {
    let mut stmt = conn.prepare(
        "SELECT version, name, checksum, applied_at FROM schema_migrations ORDER BY version",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AppliedMigration {
            version: r.get(0)?,
            name: r.get(1)?,
            checksum: r.get(2)?,
            applied_at: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn m(file: &'static str) -> Migration {
        Migration { file, sql: "" }
    }

    #[test]
    fn parses_file_names() {
        assert_eq!(m("V001__init.sql").parse_file_name().unwrap(), (1, "init"));
        assert_eq!(
            m("V0123__add_runs_2.sql").parse_file_name().unwrap(),
            (123, "add_runs_2")
        );
        for bad in [
            "V001__init.txt",
            "001__init.sql",
            "V01__init.sql",
            "V000__init.sql",
            "V001_init.sql",
            "V001__.sql",
            "V001__Init.sql",
            "V00a__init.sql",
        ] {
            assert!(m(bad).parse_file_name().is_err(), "{bad}");
        }
    }

    #[test]
    fn checksum_is_sha256_hex() {
        let sql = Migration {
            file: "V001__a.sql",
            sql: "abc",
        };
        assert_eq!(
            sql.checksum(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn every_migration_file_is_registered() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let mut on_disk: Vec<String> = match std::fs::read_dir(&dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".sql"))
                .collect(),
            Err(_) => Vec::new(),
        };
        on_disk.sort();
        let registered: Vec<String> = MIGRATIONS.iter().map(|m| m.file.to_owned()).collect();
        assert_eq!(registered, on_disk);
    }
}
