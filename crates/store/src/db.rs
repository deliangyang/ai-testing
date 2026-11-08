use std::path::Path;
use std::time::Duration;

use common::clock::Clock;
use rusqlite::Connection;

use crate::error::Result;
use crate::migrate::{self, Migration, MIGRATIONS};

/// How long a writer waits for another connection's lock before failing with `SQLITE_BUSY`.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// A configured and fully migrated SQLite connection.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Opens (creating if needed) the database file and applies pending migrations.
    pub fn open(path: impl AsRef<Path>, clock: &dyn Clock) -> Result<Self> {
        Self::open_with(path, MIGRATIONS, clock)
    }

    pub fn open_with(
        path: impl AsRef<Path>,
        migrations: &[Migration],
        clock: &dyn Clock,
    ) -> Result<Self> {
        Self::setup(Connection::open(path)?, migrations, clock)
    }

    /// Private in-memory database; WAL does not apply.
    pub fn open_in_memory(clock: &dyn Clock) -> Result<Self> {
        Self::setup(Connection::open_in_memory()?, MIGRATIONS, clock)
    }

    fn setup(mut conn: Connection, migrations: &[Migration], clock: &dyn Clock) -> Result<Self> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        enable_wal(&conn)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")?;
        migrate::migrate(&mut conn, migrations, clock)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Highest applied migration version, 0 for an empty database.
    pub fn schema_version(&self) -> Result<u32> {
        Ok(migrate::applied(&self.conn)?
            .last()
            .map_or(0, |a| a.version))
    }
}

const WAL_ATTEMPTS: usize = 20;

/// Switching the journal mode needs an exclusive lock and fails with `SQLITE_BUSY` without
/// consulting the busy handler, so concurrent first opens would otherwise fail at once.
fn enable_wal(conn: &Connection) -> Result<()> {
    let mut attempt = 0;
    loop {
        let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        if mode.eq_ignore_ascii_case("wal") || mode.eq_ignore_ascii_case("memory") {
            return Ok(());
        }
        match conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(())) {
            Ok(()) => return Ok(()),
            Err(e) if is_busy(&e) && attempt < WAL_ATTEMPTS => {
                attempt += 1;
                // Waits for the lock holder through the busy handler.
                conn.execute_batch("BEGIN IMMEDIATE; ROLLBACK;")?;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn is_busy(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}
