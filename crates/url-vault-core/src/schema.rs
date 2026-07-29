use rusqlite::Connection;

use crate::error::Result;

pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS bookmarks (
          id TEXT PRIMARY KEY,
          url TEXT NOT NULL,
          canonical_url TEXT UNIQUE NOT NULL,
          title TEXT,
          description TEXT,
          note TEXT,
          tags_json TEXT NOT NULL DEFAULT '[]',
          aliases_json TEXT NOT NULL DEFAULT '[]',
          intents_json TEXT NOT NULL DEFAULT '[]',
          source_type TEXT NOT NULL DEFAULT 'manual',
          source_browser TEXT,
          source_profile TEXT,
          folder_path TEXT,
          preferred_browser TEXT,
          project TEXT,
          status TEXT NOT NULL DEFAULT 'active',
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          first_seen_at TEXT,
          last_seen_at TEXT,
          last_opened_at TEXT,
          open_count INTEGER NOT NULL DEFAULT 0,
          seen_in_latest_import INTEGER NOT NULL DEFAULT 1,
          missing_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS imports (
          id TEXT PRIMARY KEY,
          source_browser TEXT,
          source_profile TEXT,
          file_path TEXT NOT NULL,
          imported_at TEXT NOT NULL,
          checksum TEXT NOT NULL,
          item_count INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS bookmark_imports (
          bookmark_id TEXT NOT NULL,
          import_id TEXT NOT NULL,
          folder_path TEXT,
          imported_title TEXT,
          add_date TEXT,
          last_modified TEXT,
          seen_at TEXT NOT NULL,
          PRIMARY KEY (bookmark_id, import_id),
          FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE,
          FOREIGN KEY (import_id) REFERENCES imports(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS opens (
          id TEXT PRIMARY KEY,
          bookmark_id TEXT,
          url TEXT NOT NULL,
          opened_browser TEXT NOT NULL,
          opened_by TEXT NOT NULL,
          opened_at TEXT NOT NULL,
          context TEXT,
          FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS categories (
          path TEXT PRIMARY KEY,
          label TEXT,
          note TEXT,
          status TEXT NOT NULL DEFAULT 'active',
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS snapshots (
          bookmark_id TEXT PRIMARY KEY,
          artifact_path TEXT NOT NULL,
          content_sha256 TEXT NOT NULL,
          kind TEXT NOT NULL,
          title TEXT,
          captured_at TEXT,
          byte_count INTEGER,
          content_text TEXT,
          status TEXT NOT NULL,
          verified_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS vault_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_bookmarks_canonical_url ON bookmarks(canonical_url);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_source ON bookmarks(source_browser, source_profile);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_updated ON bookmarks(updated_at);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_folder_path ON bookmarks(folder_path);
        CREATE INDEX IF NOT EXISTS idx_categories_status ON categories(status);
        CREATE INDEX IF NOT EXISTS idx_snapshots_status ON snapshots(status);
        "#,
    )?;
    ensure_column(
        conn,
        "bookmarks",
        "aliases_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "bookmarks",
        "intents_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO vault_meta(key, value) VALUES ('export_generation', '0')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO vault_meta(key, value) VALUES ('export_pending', '0')",
        [],
    )?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !names.iter().any(|name| name == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}
