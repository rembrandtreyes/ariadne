pub mod query;
pub mod schema;
pub mod write;

use rusqlite::Connection;
use std::path::Path;

/// Resolution status for unresolved calls (not yet matched to a callee symbol).
pub const RESOLUTION_UNRESOLVED: &str = "unresolved";

/// Resolution status for resolved calls (successfully matched to a callee symbol).
pub const RESOLUTION_RESOLVED: &str = "resolved";

/// Database handle wrapping a SQLite connection for the Ariadne graph store.
/// Escape special characters in a string for use in SQL LIKE patterns.
/// The escaped string should be used with `ESCAPE '\'` in the SQL query.
pub fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create the database at the given path.
    ///
    /// Creates the schema if the database is new.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
        )?;
        schema::create_tables(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (useful for testing).
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        schema::create_tables(&conn)?;
        Ok(Self { conn })
    }

    /// Get a reference to the underlying connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Alias for `conn()` for backwards compatibility.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Store a metadata key-value pair.
    pub fn set_metadata(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    /// Retrieve a metadata value by key.
    pub fn get_metadata(&self, key: &str) -> anyhow::Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM metadata WHERE key = ?1")?;
        let mut rows = stmt.query(rusqlite::params![key])?;
        match rows.next()? {
            Some(row) => {
                let value: String = row.get(0)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_creates_schema() {
        let db = Database::open_in_memory().expect("should open in-memory db");
        let mut stmt = db
            .connection()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .expect("should prepare query");
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .expect("should query")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(tables.contains(&"files".to_string()));
        assert!(tables.contains(&"symbols".to_string()));
        assert!(tables.contains(&"calls".to_string()));
        assert!(tables.contains(&"imports".to_string()));
        assert!(tables.contains(&"services".to_string()));
    }

    #[test]
    fn metadata_round_trip() {
        let db = Database::open_in_memory().expect("should open in-memory db");
        db.set_metadata("version", "0.1.0")
            .expect("should set metadata");
        let value = db
            .get_metadata("version")
            .expect("should get metadata")
            .expect("should have value");
        assert_eq!(value, "0.1.0");
    }
}
