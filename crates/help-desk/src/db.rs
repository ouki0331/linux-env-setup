use crate::models::{CommandDoc, OptionDoc, ExampleDoc};
use rusqlite::{params, Connection};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const EMBEDDED_SEED_JSON: &str = include_str!("seed.json");

pub fn get_db_path() -> PathBuf {
    if let Ok(p) = env::var("HELP_DESK_DB") {
        return PathBuf::from(p);
    }
    if let Some(home) = dirs_home() {
        let p = home.join(".help_desk.db");
        // Test write access or fallback
        if let Ok(_) = Connection::open(&p) {
            return p;
        }
    }
    PathBuf::from(".help_desk.db")
}

fn dirs_home() -> Option<PathBuf> {
    env::var("HOME").ok().map(PathBuf::from)
}

pub fn get_connection() -> rusqlite::Result<Connection> {
    let path = get_db_path();
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS commands (
            command TEXT PRIMARY KEY,
            category TEXT DEFAULT '',
            category_ja TEXT DEFAULT '',
            category_en TEXT DEFAULT '',
            summary_ja TEXT DEFAULT '',
            summary_en TEXT DEFAULT '',
            description_ja TEXT DEFAULT '',
            description_en TEXT DEFAULT '',
            syntax TEXT DEFAULT '',
            options_json TEXT,
            examples_json TEXT,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );"
    )?;
    // Ensure category column exists in case created previously
    let _ = conn.execute("ALTER TABLE commands ADD COLUMN category TEXT DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE commands ADD COLUMN category_ja TEXT DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE commands ADD COLUMN category_en TEXT DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE commands ADD COLUMN summary_en TEXT DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE commands ADD COLUMN description_en TEXT DEFAULT ''", []);
    Ok(conn)
}

pub fn auto_seed_if_empty(conn: &Connection) -> rusqlite::Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))?;
    if count == 0 {
        let _ = import_from_json_string(conn, EMBEDDED_SEED_JSON);
    }
    Ok(())
}

pub fn load_all_commands(conn: &Connection) -> Vec<CommandDoc> {
    let mut stmt = match conn.prepare(
        "SELECT command, category_ja, category_en, summary_ja, summary_en, description_ja, description_en, syntax, options_json, examples_json, updated_at FROM commands ORDER BY command ASC"
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let rows = stmt.query_map([], |row| {
        let options_raw: String = row.get(8)?;
        let examples_raw: String = row.get(9)?;

        let options: Vec<OptionDoc> = serde_json::from_str(&options_raw).unwrap_or_default();
        let examples: Vec<ExampleDoc> = serde_json::from_str(&examples_raw).unwrap_or_default();

        Ok(CommandDoc {
            command: row.get(0)?,
            category_ja: row.get(1)?,
            category_en: row.get(2)?,
            summary_ja: row.get(3)?,
            summary_en: row.get(4)?,
            description_ja: row.get(5)?,
            description_en: row.get(6)?,
            syntax: row.get(7)?,
            options,
            examples,
            updated_at: row.get(10)?,
        })
    });

    match rows {
        Ok(mapped) => mapped.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn import_from_json_string(conn: &Connection, json_str: &str) -> Result<usize, String> {
    let items: Vec<serde_json::Value> = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON parsing failed: {}", e))?;

    let mut imported = 0;
    for item in items {
        let cmd = match item.get("command").and_then(|v| v.as_str()) {
            Some(c) if !c.trim().is_empty() => c.trim(),
            _ => continue,
        };

        let cat_ja = item.get("category_ja").and_then(|v| v.as_str())
            .or_else(|| item.get("category").and_then(|v| v.as_str()))
            .unwrap_or("General");
        let cat_en = item.get("category_en").and_then(|v| v.as_str())
            .unwrap_or(cat_ja);

        let sum_ja = item.get("summary_ja").and_then(|v| v.as_str())
            .or_else(|| item.get("summary").and_then(|v| v.as_str()))
            .unwrap_or("");
        let sum_en = item.get("summary_en").and_then(|v| v.as_str())
            .unwrap_or(sum_ja);

        let desc_ja = item.get("description_ja").and_then(|v| v.as_str())
            .or_else(|| item.get("description").and_then(|v| v.as_str()))
            .unwrap_or("");
        let desc_en = item.get("description_en").and_then(|v| v.as_str())
            .unwrap_or(desc_ja);

        let syntax = item.get("syntax").and_then(|v| v.as_str()).unwrap_or("");
        let options_json = item.get("options").map(|v| v.to_string()).unwrap_or_else(|| "[]".to_string());
        let examples_json = item.get("examples").map(|v| v.to_string()).unwrap_or_else(|| "[]".to_string());

        let res = conn.execute(
            "INSERT INTO commands (command, category, category_ja, category_en, summary_ja, summary_en, description_ja, description_en, syntax, options_json, examples_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, CURRENT_TIMESTAMP)
             ON CONFLICT(command) DO UPDATE SET
                category = excluded.category,
                category_ja = excluded.category_ja,
                category_en = excluded.category_en,
                summary_ja = excluded.summary_ja,
                summary_en = excluded.summary_en,
                description_ja = excluded.description_ja,
                description_en = excluded.description_en,
                syntax = excluded.syntax,
                options_json = excluded.options_json,
                examples_json = excluded.examples_json,
                updated_at = CURRENT_TIMESTAMP;",
            params![cmd, cat_ja, cat_ja, cat_en, sum_ja, sum_en, desc_ja, desc_en, syntax, options_json, examples_json],
        );

        if res.is_ok() {
            imported += 1;
        }
    }
    Ok(imported)
}

pub fn import_from_file(conn: &Connection, file_path: &str) -> Result<usize, String> {
    let p = Path::new(file_path);
    if !p.exists() {
        return Err(format!("File not found: {}", file_path));
    }
    let content = fs::read_to_string(p)
        .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
    import_from_json_string(conn, &content)
}
