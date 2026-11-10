//! Database persistence with SQLite and transactional integrity.

use crate::catalog::{CatalogEntry, Example, Provider, ReviewState, SourceEvidence};
use crate::error::{Error, Result};
use crate::paths::Paths;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::fs;
use std::path::Path;

pub fn open_db(paths: &Paths) -> Result<Connection> {
    open_db_at(&paths.db_path())
}

pub fn open_db_at(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::io(
                format!("Failed to create database directory {}", parent.display()),
                e,
            )
        })?;
    }

    let conn = Connection::open(db_path).map_err(|e| {
        Error::Db(format!(
            "Failed to open SQLite database at {}: {e}",
            db_path.display()
        ))
    })?;

    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;

         CREATE TABLE IF NOT EXISTS catalog_entries (
             command_id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             kind TEXT NOT NULL,
             summary TEXT NOT NULL DEFAULT '',
             syntax TEXT NOT NULL DEFAULT '',
             examples_json TEXT NOT NULL DEFAULT '[]',
             notes_json TEXT NOT NULL DEFAULT '[]',
             tags_json TEXT NOT NULL DEFAULT '[]',
             provider_ecosystem TEXT,
             provider_package TEXT,
             provider_version TEXT,
             documented_against_version TEXT,
             source_json TEXT NOT NULL DEFAULT '{}',
             updated_at TEXT NOT NULL,
             review_state TEXT NOT NULL DEFAULT 'documented'
         );

         CREATE INDEX IF NOT EXISTS idx_catalog_name ON catalog_entries(name);
         CREATE INDEX IF NOT EXISTS idx_catalog_kind ON catalog_entries(kind);

         CREATE TABLE IF NOT EXISTS snapshots (
             snapshot_id TEXT PRIMARY KEY,
             generated_at TEXT NOT NULL,
             complete INTEGER NOT NULL,
             host_os TEXT NOT NULL,
             host_arch TEXT NOT NULL,
             host_shell TEXT NOT NULL,
             inventory_json TEXT NOT NULL
         );

         CREATE INDEX IF NOT EXISTS idx_snapshots_date ON snapshots(generated_at DESC);",
    )?;

    Ok(conn)
}

/// Retrieve a catalog entry by either command_id or command name.
pub fn get_entry(conn: &Connection, command_or_id: &str) -> Result<Option<CatalogEntry>> {
    let q = "SELECT command_id, name, kind, summary, syntax, examples_json, notes_json, tags_json,
                    provider_ecosystem, provider_package, provider_version,
                    documented_against_version, source_json, updated_at, review_state
             FROM catalog_entries
             WHERE command_id = ?1 OR name = ?1
             ORDER BY CASE WHEN command_id = ?1 THEN 0 ELSE 1 END
             LIMIT 1";

    let mut stmt = conn.prepare(q)?;
    let entry = stmt
        .query_row(params![command_or_id], row_to_entry)
        .optional()?;

    Ok(entry)
}

/// List all catalog entries, optionally filtering by kind and/or source ecosystem.
pub fn list_entries(
    conn: &Connection,
    kind_filter: Option<&str>,
    source_filter: Option<&str>,
) -> Result<Vec<CatalogEntry>> {
    let mut sql = String::from(
        "SELECT command_id, name, kind, summary, syntax, examples_json, notes_json, tags_json,
                provider_ecosystem, provider_package, provider_version,
                documented_against_version, source_json, updated_at, review_state
         FROM catalog_entries WHERE 1=1",
    );

    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(kind) = kind_filter {
        sql.push_str(" AND LOWER(kind) = LOWER(?)");
        params_vec.push(Box::new(kind.to_string()));
    }

    if let Some(source) = source_filter {
        sql.push_str(
            " AND (LOWER(provider_ecosystem) = LOWER(?) OR LOWER(command_id) LIKE LOWER(?))",
        );
        params_vec.push(Box::new(source.to_string()));
        params_vec.push(Box::new(format!("{source}:%")));
    }

    sql.push_str(" ORDER BY name ASC, command_id ASC");

    let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(&params_slice[..], row_to_entry)?;

    let mut entries = Vec::new();
    for r in rows {
        entries.push(r?);
    }
    Ok(entries)
}

/// Search catalog entries by keyword matching name, summary, tags, notes, or syntax.
pub fn search_entries(conn: &Connection, keyword: &str) -> Result<Vec<CatalogEntry>> {
    let kw = format!("%{}%", keyword.trim().to_lowercase());
    let sql =
        "SELECT command_id, name, kind, summary, syntax, examples_json, notes_json, tags_json,
                      provider_ecosystem, provider_package, provider_version,
                      documented_against_version, source_json, updated_at, review_state
               FROM catalog_entries
               WHERE LOWER(name) LIKE ?1
                  OR LOWER(summary) LIKE ?1
                  OR LOWER(syntax) LIKE ?1
                  OR LOWER(tags_json) LIKE ?1
                  OR LOWER(notes_json) LIKE ?1
               ORDER BY name ASC, command_id ASC";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![kw], row_to_entry)?;

    let mut entries = Vec::new();
    for r in rows {
        entries.push(r?);
    }
    Ok(entries)
}

/// Insert or update a catalog entry.
pub fn save_entry(conn: &Connection, entry: &CatalogEntry) -> Result<()> {
    let examples_json = serde_json::to_string(&entry.examples)?;
    let notes_json = serde_json::to_string(&entry.notes)?;
    let tags_json = serde_json::to_string(&entry.tags)?;
    let source_json = serde_json::to_string(&entry.source)?;

    let (eco, pkg, ver) = match &entry.provider {
        Some(p) => (Some(&p.ecosystem), Some(&p.package), p.version.as_ref()),
        None => (None, None, None),
    };

    conn.execute(
        "INSERT INTO catalog_entries (
             command_id, name, kind, summary, syntax, examples_json, notes_json, tags_json,
             provider_ecosystem, provider_package, provider_version,
             documented_against_version, source_json, updated_at, review_state
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT(command_id) DO UPDATE SET
             name = excluded.name,
             kind = excluded.kind,
             summary = excluded.summary,
             syntax = excluded.syntax,
             examples_json = excluded.examples_json,
             notes_json = excluded.notes_json,
             tags_json = excluded.tags_json,
             provider_ecosystem = excluded.provider_ecosystem,
             provider_package = excluded.provider_package,
             provider_version = excluded.provider_version,
             documented_against_version = excluded.documented_against_version,
             source_json = excluded.source_json,
             updated_at = excluded.updated_at,
             review_state = excluded.review_state",
        params![
            entry.command_id,
            entry.name,
            entry.kind,
            entry.summary,
            entry.syntax,
            examples_json,
            notes_json,
            tags_json,
            eco,
            pkg,
            ver,
            entry.documented_against_version,
            source_json,
            entry.updated_at,
            entry.review_state.as_str(),
        ],
    )?;

    Ok(())
}

/// Delete a catalog entry by command_id or command name.
pub fn delete_entry(conn: &Connection, command_or_id: &str) -> Result<bool> {
    let rows_affected = conn.execute(
        "DELETE FROM catalog_entries WHERE command_id = ?1 OR name = ?1",
        params![command_or_id],
    )?;
    Ok(rows_affected > 0)
}

/// Import a batch of catalog entries transactionally.
/// If `allowed_ids` is provided, any entry not in `allowed_ids` is rejected.
pub fn import_entries_tx(
    conn: &mut Connection,
    entries: &[CatalogEntry],
    allowed_ids: Option<&[String]>,
) -> Result<usize> {
    let tx = conn.transaction()?;

    let mut count = 0;
    for entry in entries {
        if let Some(allowed) = allowed_ids {
            if !allowed.iter().any(|id| id == &entry.command_id) {
                return Err(Error::Validation(vec![format!(
                    "Command ID '{}' is outside the targeted changes-only update scope",
                    entry.command_id
                )]));
            }
        }

        save_entry_in_tx(&tx, entry)?;
        count += 1;
    }

    tx.commit()?;
    Ok(count)
}

fn save_entry_in_tx(tx: &Transaction<'_>, entry: &CatalogEntry) -> Result<()> {
    let examples_json = serde_json::to_string(&entry.examples)?;
    let notes_json = serde_json::to_string(&entry.notes)?;
    let tags_json = serde_json::to_string(&entry.tags)?;
    let source_json = serde_json::to_string(&entry.source)?;

    let (eco, pkg, ver) = match &entry.provider {
        Some(p) => (Some(&p.ecosystem), Some(&p.package), p.version.as_ref()),
        None => (None, None, None),
    };

    tx.execute(
        "INSERT INTO catalog_entries (
             command_id, name, kind, summary, syntax, examples_json, notes_json, tags_json,
             provider_ecosystem, provider_package, provider_version,
             documented_against_version, source_json, updated_at, review_state
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT(command_id) DO UPDATE SET
             name = excluded.name,
             kind = excluded.kind,
             summary = excluded.summary,
             syntax = excluded.syntax,
             examples_json = excluded.examples_json,
             notes_json = excluded.notes_json,
             tags_json = excluded.tags_json,
             provider_ecosystem = excluded.provider_ecosystem,
             provider_package = excluded.provider_package,
             provider_version = excluded.provider_version,
             documented_against_version = excluded.documented_against_version,
             source_json = excluded.source_json,
             updated_at = excluded.updated_at,
             review_state = excluded.review_state",
        params![
            entry.command_id,
            entry.name,
            entry.kind,
            entry.summary,
            entry.syntax,
            examples_json,
            notes_json,
            tags_json,
            eco,
            pkg,
            ver,
            entry.documented_against_version,
            source_json,
            entry.updated_at,
            entry.review_state.as_str(),
        ],
    )?;

    Ok(())
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<CatalogEntry> {
    let command_id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let kind: String = row.get(2)?;
    let summary: String = row.get(3)?;
    let syntax: String = row.get(4)?;
    let examples_raw: String = row.get(5)?;
    let notes_raw: String = row.get(6)?;
    let tags_raw: String = row.get(7)?;
    let eco: Option<String> = row.get(8)?;
    let pkg: Option<String> = row.get(9)?;
    let ver: Option<String> = row.get(10)?;
    let documented_against: Option<String> = row.get(11)?;
    let source_raw: String = row.get(12)?;
    let updated_at: String = row.get(13)?;
    let state_raw: String = row.get(14)?;

    let examples: Vec<Example> = serde_json::from_str(&examples_raw).unwrap_or_default();
    let notes: Vec<String> = serde_json::from_str(&notes_raw).unwrap_or_default();
    let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
    let source: Option<SourceEvidence> = serde_json::from_str(&source_raw).ok();

    let provider = match (eco, pkg) {
        (Some(e), Some(p)) => Some(Provider {
            ecosystem: e,
            package: p,
            version: ver,
        }),
        _ => None,
    };

    Ok(CatalogEntry {
        command_id,
        name,
        kind,
        summary,
        syntax,
        examples,
        notes,
        tags,
        provider,
        documented_against_version: documented_against,
        source,
        updated_at,
        review_state: ReviewState::from_str_lossy(&state_raw),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_crud_and_transaction_rollback() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_catalog.db");
        let mut conn = open_db_at(&db_path).unwrap();

        let mut entry = CatalogEntry::new("cargo:ripgrep:rg", "rg", "rust_binary");
        entry.summary = "Fast regex search".into();
        entry.syntax = "rg [OPTIONS] PATTERN".into();
        entry.tags = vec!["search".into()];

        save_entry(&conn, &entry).unwrap();

        let loaded = get_entry(&conn, "rg").unwrap().unwrap();
        assert_eq!(loaded.command_id, "cargo:ripgrep:rg");
        assert_eq!(loaded.summary, "Fast regex search");

        let search_res = search_entries(&conn, "regex").unwrap();
        assert_eq!(search_res.len(), 1);

        // Test transaction rollback on failure
        let invalid_entry = CatalogEntry::new("bad:item", "bad", "binary");
        let allowed = vec!["other:item".to_string()];
        let res = import_entries_tx(&mut conn, &[invalid_entry], Some(&allowed));
        assert!(res.is_err());

        // Ensure database remains unchanged
        assert!(get_entry(&conn, "bad").unwrap().is_none());

        // Test delete
        let deleted = delete_entry(&conn, "rg").unwrap();
        assert!(deleted);
        assert!(get_entry(&conn, "rg").unwrap().is_none());
    }
}
