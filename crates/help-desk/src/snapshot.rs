//! Immutable snapshot persistence and retrieval.

use crate::error::{Error, Result};
use crate::inventory::Inventory;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub snapshot_id: String,
    pub generated_at: String,
    pub complete: bool,
    pub host_os: String,
    pub host_arch: String,
    pub package_count: usize,
    pub command_count: usize,
}

pub fn save_snapshot(conn: &Connection, inv: &Inventory) -> Result<()> {
    let json_str = serde_json::to_string_pretty(inv)?;
    conn.execute(
        "INSERT INTO snapshots (
             snapshot_id, generated_at, complete, host_os, host_arch, host_shell, inventory_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(snapshot_id) DO UPDATE SET
             generated_at = excluded.generated_at,
             complete = excluded.complete,
             inventory_json = excluded.inventory_json",
        params![
            inv.snapshot_id,
            inv.generated_at,
            if inv.complete { 1 } else { 0 },
            inv.host.os,
            inv.host.arch,
            inv.host.shell,
            json_str,
        ],
    )?;
    Ok(())
}

pub fn get_snapshot(conn: &Connection, snapshot_id: &str) -> Result<Option<Inventory>> {
    let mut stmt = conn.prepare("SELECT inventory_json FROM snapshots WHERE snapshot_id = ?1")?;
    let json: Option<String> = stmt
        .query_row(params![snapshot_id], |row| row.get(0))
        .optional()?;

    match json {
        Some(s) => {
            let inv: Inventory = serde_json::from_str(&s)?;
            Ok(Some(inv))
        }
        None => Ok(None),
    }
}

pub fn list_snapshots(conn: &Connection) -> Result<Vec<SnapshotMeta>> {
    let mut stmt = conn.prepare(
        "SELECT snapshot_id, generated_at, complete, host_os, host_arch, inventory_json
         FROM snapshots
         ORDER BY generated_at DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        let snapshot_id: String = row.get(0)?;
        let generated_at: String = row.get(1)?;
        let complete_int: i32 = row.get(2)?;
        let host_os: String = row.get(3)?;
        let host_arch: String = row.get(4)?;
        let json_str: String = row.get(5)?;

        let inv: Inventory = serde_json::from_str(&json_str)
            .unwrap_or_else(|_| Inventory::new(snapshot_id.clone(), complete_int == 1));

        Ok(SnapshotMeta {
            snapshot_id,
            generated_at,
            complete: complete_int == 1,
            host_os,
            host_arch,
            package_count: inv.packages.len(),
            command_count: inv.commands.len(),
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn get_recent_complete_snapshots(conn: &Connection, limit: usize) -> Result<Vec<Inventory>> {
    let mut stmt = conn.prepare(
        "SELECT inventory_json FROM snapshots WHERE complete = 1 ORDER BY generated_at DESC LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit as i64], |row| {
        let json: String = row.get(0)?;
        let inv: Inventory = serde_json::from_str(&json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(inv)
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn prune_snapshots(conn: &Connection, keep: usize) -> Result<usize> {
    if keep == 0 {
        return Err(Error::Usage("--keep must be at least 1".into()));
    }

    let all = list_snapshots(conn)?;
    if all.len() <= keep {
        return Ok(0);
    }

    let to_remove = &all[keep..];
    let mut count = 0;
    for s in to_remove {
        conn.execute(
            "DELETE FROM snapshots WHERE snapshot_id = ?1",
            params![s.snapshot_id],
        )?;
        count += 1;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_db_at;
    use tempfile::tempdir;

    #[test]
    fn test_snapshot_lifecycle() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_snapshots.db");
        let conn = open_db_at(&db_path).unwrap();

        let inv = Inventory::new("snap-1".into(), true);
        save_snapshot(&conn, &inv).unwrap();

        let loaded = get_snapshot(&conn, "snap-1").unwrap().unwrap();
        assert_eq!(loaded.snapshot_id, "snap-1");
        assert!(loaded.complete);

        let list = list_snapshots(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].snapshot_id, "snap-1");

        let pruned = prune_snapshots(&conn, 1).unwrap();
        assert_eq!(pruned, 0);
    }
}
