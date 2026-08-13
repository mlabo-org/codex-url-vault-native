use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, Utc};
use fs2::FileExt;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};

use crate::error::{Result, VaultError};
use crate::html::{hex_sha256, parse_bookmark_html, render_bookmarks_html};
use crate::model::{
    ApplyImportInput, Bookmark, BookmarkIdentity, Category, ImportResult, InitResult,
    MutationResult, OpenResult, SaveOperation, SaveSnapshotInput, SaveUrlInput, SaveUrlResult,
    Snapshot, SnapshotMetadata, SnapshotPayload, SnapshotResult, SnapshotVerification,
    SuggestResult, UpdateUrlInput,
};
use crate::{schema, search};

const DEFAULT_HOME: &str = "~/.codex/url-vault";
const DEFAULT_APP_PATH: &str = "~/Applications/Codex URL Vault.app";
const BOOKMARK_SELECT: &str = r#"
    SELECT b.*,
           s.bookmark_id AS snapshot_bookmark_id,
           s.artifact_path AS snapshot_artifact_path,
           s.content_sha256 AS snapshot_sha256,
           s.kind AS snapshot_kind,
           s.title AS snapshot_title,
           s.captured_at AS snapshot_captured_at,
           s.byte_count AS snapshot_byte_count,
           s.status AS snapshot_status,
           s.verified_at AS snapshot_verified_at,
           s.updated_at AS snapshot_updated_at,
           s.content_text AS snapshot_content_text
      FROM bookmarks b
      LEFT JOIN snapshots s ON s.bookmark_id = b.id
"#;

#[derive(Debug, Clone)]
pub struct Vault {
    home: PathBuf,
}

impl Vault {
    pub fn from_env() -> Result<Self> {
        let configured = std::env::var_os("CODEX_URL_VAULT_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| expand_home(DEFAULT_HOME));
        Self::at(configured)
    }

    pub fn at(home: impl Into<PathBuf>) -> Result<Self> {
        let home = home.into();
        if home.as_os_str().is_empty() {
            return Err(VaultError::InvalidInput(
                "Vault home must not be empty".to_owned(),
            ));
        }
        Ok(Self { home })
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn db_path(&self) -> PathBuf {
        self.home.join("vault.sqlite")
    }

    pub fn canonical_html_path(&self) -> PathBuf {
        self.home.join("current-bookmarks.html")
    }

    pub fn init(&self) -> Result<InitResult> {
        let conn = self.connection()?;
        let count = conn.query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))?;
        Ok(InitResult {
            db: self.db_path().display().to_string(),
            bookmarks: count,
        })
    }

    pub fn list_urls(&self, category: Option<&str>, limit: Option<usize>) -> Result<Vec<Bookmark>> {
        let bookmarks = self.query_urls(category, limit)?;
        self.verify_bookmark_snapshots(bookmarks, false)
    }

    fn query_urls(&self, category: Option<&str>, limit: Option<usize>) -> Result<Vec<Bookmark>> {
        let conn = self.connection()?;
        let mut sql = format!("{BOOKMARK_SELECT} WHERE b.status = 'active'");
        let normalized = category.map(|value| normalize_category_path(Some(value)));
        if normalized.is_some() {
            sql.push_str(" AND COALESCE(b.folder_path, '') = ?1");
        }
        sql.push_str(" ORDER BY b.folder_path, lower(COALESCE(b.title, b.url)), b.url");
        let mut statement = conn.prepare(&sql)?;
        let mut values = if let Some(category) = normalized {
            statement
                .query_map([category], bookmark_from_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            statement
                .query_map([], bookmark_from_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        if let Some(limit) = limit {
            values.truncate(limit);
        }
        Ok(values)
    }

    pub fn list_categories(&self) -> Result<Vec<Category>> {
        let conn = self.connection()?;
        let mut counts = HashMap::<String, i64>::new();
        let mut count_statement = conn.prepare(
            r#"
            SELECT COALESCE(folder_path, '') AS path, COUNT(*) AS count
              FROM bookmarks
             WHERE status = 'active'
             GROUP BY COALESCE(folder_path, '')
            "#,
        )?;
        for row in count_statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })? {
            let (path, count) = row?;
            counts.insert(path, count);
        }

        let mut categories = Vec::new();
        let mut explicit = HashMap::<String, Category>::new();
        let mut statement = conn.prepare(
            "SELECT path, label, note, status, created_at, updated_at FROM categories WHERE status = 'active'",
        )?;
        for row in statement.query_map([], |row| {
            let path = row.get::<_, String>(0)?;
            Ok(Category {
                display: category_display(&path),
                label: row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| category_display(&path)),
                note: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                count: 0,
                path,
            })
        })? {
            let category = row?;
            explicit.insert(category.path.clone(), category);
        }
        for path in counts.keys() {
            explicit.entry(path.clone()).or_insert_with(|| Category {
                path: path.clone(),
                label: category_display(path),
                note: None,
                status: "active".to_owned(),
                created_at: None,
                updated_at: None,
                display: category_display(path),
                count: 0,
            });
        }
        for (_, mut category) in explicit {
            category.count = *counts.get(&category.path).unwrap_or(&0);
            categories.push(category);
        }
        categories.sort_by(|left, right| {
            right.count.cmp(&left.count).then_with(|| {
                left.display
                    .to_lowercase()
                    .cmp(&right.display.to_lowercase())
            })
        });
        Ok(categories)
    }

    pub fn search_urls(&self, query: &str, limit: usize) -> Result<Vec<Bookmark>> {
        let bookmarks = self.list_urls_with_snapshot_content()?;
        Ok(search::search(bookmarks, query, limit)
            .into_iter()
            .map(redact_snapshot_content)
            .collect())
    }

    pub fn suggest_urls(&self, query: &str, limit: usize) -> Result<SuggestResult> {
        let bookmarks = self.list_urls_with_snapshot_content()?;
        let mut result = search::suggest(bookmarks, query, limit);
        result.candidates = result
            .candidates
            .into_iter()
            .map(redact_snapshot_content)
            .collect();
        Ok(result)
    }

    pub fn get_url(&self, target: &str) -> Result<Bookmark> {
        let conn = self.connection()?;
        self.resolve_bookmark(&conn, target, 5)
    }

    pub fn save_url(&self, input: SaveUrlInput) -> Result<MutationResult<Bookmark>> {
        let result = self.save_url_with_operation(input)?;
        Ok(MutationResult {
            item: result.item,
            canonical_html: result.canonical_html,
        })
    }

    pub fn save_url_with_operation(&self, input: SaveUrlInput) -> Result<SaveUrlResult> {
        if input.url.trim().is_empty() {
            return Err(VaultError::InvalidInput("url is required".to_owned()));
        }
        let result = self.mutate(|transaction| {
            let canonical = canonicalize_url(&input.url);
            let existed = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM bookmarks WHERE canonical_url = ?1)",
                [&canonical],
                |row| row.get::<_, i64>(0),
            )? != 0;
            let bookmark = upsert_bookmark(transaction, &input)?;
            if let Some(folder) = input
                .folder_path
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                upsert_category(transaction, folder, None, None)?;
            }
            let bookmark = load_bookmark(transaction, &bookmark.id)?;
            let operation = if existed {
                SaveOperation::Updated
            } else {
                SaveOperation::Created
            };
            Ok((bookmark, operation))
        })?;
        Ok(SaveUrlResult {
            item: result.item.0,
            canonical_html: result.canonical_html,
            operation: result.item.1,
        })
    }

    pub fn update_url(
        &self,
        target: &str,
        input: UpdateUrlInput,
    ) -> Result<MutationResult<Bookmark>> {
        let target = target.to_owned();
        self.mutate(|transaction| {
            let current = resolve_bookmark_on(transaction, &target, 5)?;
            if let Some(url) = input.url.as_deref() {
                let canonical = canonicalize_url(url);
                if canonical != current.canonical_url {
                    let replacement = SaveUrlInput {
                        url: url.to_owned(),
                        title: input.title.clone().or(current.title.clone()),
                        description: input.description.clone().or(current.description.clone()),
                        note: input.note.clone().or(current.note.clone()),
                        tags: input.tags.clone().unwrap_or(current.tags.clone()),
                        aliases: input.aliases.clone().unwrap_or(current.aliases.clone()),
                        intents: input.intents.clone().unwrap_or(current.intents.clone()),
                        source_type: Some(current.source_type.clone()),
                        source_browser: current.source_browser.clone(),
                        source_profile: current.source_profile.clone(),
                        folder_path: input.folder_path.clone().or(current.folder_path.clone()),
                        preferred_browser: input
                            .preferred_browser
                            .clone()
                            .or(current.preferred_browser.clone()),
                        project: input.project.clone().or(current.project.clone()),
                        status: Some("active".to_owned()),
                    };
                    let replacement = upsert_bookmark(transaction, &replacement)?;
                    transaction.execute(
                        "UPDATE bookmarks SET status = 'replaced', updated_at = ?1 WHERE id = ?2",
                        params![now_iso(), current.id],
                    )?;
                    return load_bookmark(transaction, &replacement.id);
                }
            }

            if let Some(value) = input.title {
                transaction.execute(
                    "UPDATE bookmarks SET title = ?1, updated_at = ?2 WHERE id = ?3",
                    params![value, now_iso(), current.id],
                )?;
            }
            if let Some(value) = input.description {
                transaction.execute(
                    "UPDATE bookmarks SET description = ?1, updated_at = ?2 WHERE id = ?3",
                    params![value, now_iso(), current.id],
                )?;
            }
            if let Some(value) = input.note {
                transaction.execute(
                    "UPDATE bookmarks SET note = ?1, updated_at = ?2 WHERE id = ?3",
                    params![value, now_iso(), current.id],
                )?;
            }
            if let Some(value) = input.tags {
                set_json_list(transaction, &current.id, "tags_json", &value)?;
            }
            if let Some(value) = input.aliases {
                set_json_list(transaction, &current.id, "aliases_json", &value)?;
            }
            if let Some(value) = input.intents {
                set_json_list(transaction, &current.id, "intents_json", &value)?;
            }
            if let Some(value) = input.folder_path {
                let normalized = normalize_category_path(Some(&value));
                transaction.execute(
                    "UPDATE bookmarks SET folder_path = ?1, updated_at = ?2 WHERE id = ?3",
                    params![normalized, now_iso(), current.id],
                )?;
                if !normalized.is_empty() {
                    upsert_category(transaction, &normalized, None, None)?;
                }
            }
            if let Some(value) = input.preferred_browser {
                transaction.execute(
                    "UPDATE bookmarks SET preferred_browser = ?1, updated_at = ?2 WHERE id = ?3",
                    params![
                        normalize_browser_policy(Some(&value)),
                        now_iso(),
                        current.id
                    ],
                )?;
            }
            if let Some(value) = input.project {
                transaction.execute(
                    "UPDATE bookmarks SET project = ?1, updated_at = ?2 WHERE id = ?3",
                    params![value, now_iso(), current.id],
                )?;
            }
            load_bookmark(transaction, &current.id)
        })
    }

    pub fn move_url(&self, target: &str, category: &str) -> Result<MutationResult<Bookmark>> {
        let target = target.to_owned();
        let category = normalize_category_path(Some(category));
        self.mutate(|transaction| {
            let bookmark = resolve_bookmark_on(transaction, &target, 5)?;
            transaction.execute(
                "UPDATE bookmarks SET folder_path = ?1, updated_at = ?2 WHERE id = ?3",
                params![category, now_iso(), bookmark.id],
            )?;
            if !category.is_empty() {
                upsert_category(transaction, &category, None, None)?;
            }
            load_bookmark(transaction, &bookmark.id)
        })
    }

    pub fn archive_url(&self, target: &str) -> Result<MutationResult<Bookmark>> {
        let target = target.to_owned();
        self.mutate(|transaction| {
            let bookmark = resolve_bookmark_on(transaction, &target, 5)?;
            transaction.execute(
                "UPDATE bookmarks SET status = 'deleted', updated_at = ?1 WHERE id = ?2",
                params![now_iso(), bookmark.id],
            )?;
            load_bookmark_any_status(transaction, &bookmark.id)
        })
    }

    pub fn delete_url(&self, target: &str) -> Result<MutationResult<Bookmark>> {
        let target = target.to_owned();
        self.mutate(|transaction| {
            let bookmark = resolve_bookmark_on(transaction, &target, 5)?;
            transaction.execute("DELETE FROM bookmarks WHERE id = ?1", [&bookmark.id])?;
            Ok(bookmark)
        })
    }

    pub fn create_category(
        &self,
        path: &str,
        label: Option<&str>,
        note: Option<&str>,
    ) -> Result<MutationResult<Category>> {
        let path = path.to_owned();
        let label = label.map(str::to_owned);
        let note = note.map(str::to_owned);
        self.mutate(|transaction| {
            upsert_category(transaction, &path, label.as_deref(), note.as_deref())?;
            load_category(transaction, &normalize_category_path(Some(&path)))
        })
    }

    pub fn rename_category(
        &self,
        old_path: &str,
        new_path: &str,
    ) -> Result<MutationResult<Category>> {
        let old_path = normalize_category_path(Some(old_path));
        let new_path = normalize_category_path(Some(new_path));
        if old_path == new_path {
            return Err(VaultError::InvalidInput(
                "old and new category are the same".to_owned(),
            ));
        }
        self.mutate(|transaction| {
            upsert_category(transaction, &new_path, None, None)?;
            let timestamp = now_iso();
            transaction.execute(
                "UPDATE bookmarks SET folder_path = ?1, updated_at = ?2 WHERE COALESCE(folder_path, '') = ?3",
                params![new_path, timestamp, old_path],
            )?;
            transaction.execute(
                "UPDATE categories SET status = 'deleted', updated_at = ?1 WHERE path = ?2",
                params![timestamp, old_path],
            )?;
            load_category(transaction, &new_path)
        })
    }

    pub fn archive_category(&self, path: &str, move_to: &str) -> Result<MutationResult<Category>> {
        let path = normalize_category_path(Some(path));
        let move_to = normalize_category_path(Some(move_to));
        if path == move_to {
            return Err(VaultError::InvalidInput(
                "archive path and move target are the same".to_owned(),
            ));
        }
        self.mutate(|transaction| {
            if !move_to.is_empty() {
                upsert_category(transaction, &move_to, None, None)?;
            }
            let timestamp = now_iso();
            transaction.execute(
                "UPDATE bookmarks SET folder_path = ?1, updated_at = ?2 WHERE COALESCE(folder_path, '') = ?3",
                params![move_to, timestamp, path],
            )?;
            transaction.execute(
                r#"
                INSERT INTO categories(path, label, note, status, created_at, updated_at)
                VALUES (?1, ?2, NULL, 'deleted', ?3, ?3)
                ON CONFLICT(path) DO UPDATE SET status = 'deleted', updated_at = excluded.updated_at
                "#,
                params![path, category_display(&path), timestamp],
            )?;
            load_category_any_status(transaction, &path)
        })
    }

    pub fn preview_bookmark_import(
        &self,
        html: &str,
        file_name: &str,
        strip_common_root: bool,
    ) -> Result<crate::model::ImportPreview> {
        Ok(parse_bookmark_html(html, file_name, strip_common_root)?.preview)
    }

    pub fn apply_bookmark_import(&self, input: ApplyImportInput) -> Result<ImportResult> {
        let mode = input.mode.trim().to_lowercase();
        if !matches!(mode.as_str(), "add" | "merge" | "reset") {
            return Err(VaultError::InvalidInput(
                "unsupported_import_mode".to_owned(),
            ));
        }
        if mode == "reset" && !input.confirm_reset {
            return Err(VaultError::InvalidInput(
                "reset_confirmation_required".to_owned(),
            ));
        }
        let source_browser = input.source_browser.trim().to_lowercase();
        if source_browser.is_empty() {
            return Err(VaultError::InvalidInput(
                "missing_source_browser".to_owned(),
            ));
        }
        let parsed = parse_bookmark_html(&input.html, &input.file_name, input.strip_common_root)?;
        if input.expected_sha256 != parsed.preview.content_sha256 {
            return Err(VaultError::InvalidInput(
                "preview_checksum_mismatch".to_owned(),
            ));
        }

        let _guard = self.mutation_guard()?;
        let mut conn = self.raw_connection()?;
        self.recover_export_locked(&conn)?;
        let backup = if mode == "reset" && self.db_path().exists() {
            Some(self.backup_database(&conn, "reset")?)
        } else {
            None
        };
        let imported_at = now_iso();
        let import_id = short_hash(format!("{}{}", parsed.preview.content_sha256, imported_at));
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if mode == "reset" {
            reset_vault_data(&transaction)?;
        } else if mode == "merge" {
            transaction.execute(
                r#"
                UPDATE bookmarks
                   SET seen_in_latest_import = 0,
                       missing_count = missing_count + 1,
                       updated_at = ?1
                 WHERE source_type = 'browser_import'
                   AND COALESCE(source_browser, '') = COALESCE(?2, '')
                   AND COALESCE(source_profile, '') = COALESCE(?3, '')
                "#,
                params![imported_at, source_browser, input.source_profile.trim()],
            )?;
        }
        transaction.execute(
            r#"
            INSERT INTO imports(id, source_browser, source_profile, file_path, imported_at, checksum, item_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                import_id,
                source_browser,
                input.source_profile.trim(),
                parsed.preview.file_name,
                imported_at,
                parsed.preview.content_sha256,
                parsed.preview.parsed_count as i64
            ],
        )?;

        let mut created = 0;
        let mut updated = 0;
        let mut skipped = 0;
        for item in &parsed.items {
            let canonical = canonicalize_url(&item.url);
            let existing_id = transaction
                .query_row(
                    "SELECT id FROM bookmarks WHERE canonical_url = ?1",
                    [&canonical],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let bookmark_id = if mode == "add" {
                if let Some(existing_id) = existing_id {
                    skipped += 1;
                    existing_id
                } else {
                    let input = import_item_to_save_input(item, &source_browser, &input);
                    let bookmark = upsert_bookmark(&transaction, &input)?;
                    created += 1;
                    bookmark.id
                }
            } else {
                let save_input = import_item_to_save_input(item, &source_browser, &input);
                let bookmark = upsert_bookmark(&transaction, &save_input)?;
                if existing_id.is_some() {
                    updated += 1;
                } else {
                    created += 1;
                }
                transaction.execute(
                    r#"
                    UPDATE bookmarks
                       SET source_type = 'browser_import',
                           source_browser = ?1,
                           source_profile = ?2,
                           folder_path = ?3,
                           last_seen_at = ?4,
                           seen_in_latest_import = 1,
                           missing_count = 0
                     WHERE id = ?5
                    "#,
                    params![
                        source_browser,
                        input.source_profile.trim(),
                        normalize_category_path(Some(&item.folder_path)),
                        imported_at,
                        bookmark.id
                    ],
                )?;
                if let Some(snapshot) = &item.snapshot {
                    upsert_snapshot_reference(&transaction, &self.home, &bookmark.id, snapshot)?;
                }
                bookmark.id
            };
            transaction.execute(
                r#"
                INSERT OR REPLACE INTO bookmark_imports(
                    bookmark_id, import_id, folder_path, imported_title, add_date, last_modified, seen_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    bookmark_id,
                    import_id,
                    item.folder_path,
                    item.title,
                    item.add_date,
                    item.last_modified,
                    imported_at
                ],
            )?;
        }

        let canonical_html = self.commit_transaction(transaction)?;
        Ok(ImportResult {
            import_id,
            db: self.db_path().display().to_string(),
            mode,
            source_browser,
            source_profile: input.source_profile.trim().to_owned(),
            created,
            updated,
            skipped,
            canonical_html,
            file_name: parsed.preview.file_name,
            content_sha256: parsed.preview.content_sha256,
            parsed_count: parsed.preview.parsed_count,
            unique_count: parsed.preview.unique_count,
            duplicate_count: parsed.preview.duplicate_count,
            folder_count: parsed.preview.folder_count,
            folders: parsed.preview.folders,
            common_root: parsed.preview.common_root,
            stripped_root: parsed.preview.stripped_root,
            backup,
        })
    }

    pub fn refresh_db(&self) -> Result<ImportResult> {
        let path = self.canonical_html_path();
        let html = fs::read_to_string(&path).map_err(|error| {
            VaultError::InvalidInput(format!(
                "canonical HTML not readable at {}: {error}",
                path.display()
            ))
        })?;
        let preview = self.preview_bookmark_import(&html, "current-bookmarks.html", true)?;
        self.apply_bookmark_import(ApplyImportInput {
            html,
            file_name: "current-bookmarks.html".to_owned(),
            source_browser: "vault".to_owned(),
            source_profile: "canonical".to_owned(),
            preferred_browser: None,
            mode: "reset".to_owned(),
            strip_common_root: true,
            expected_sha256: preview.content_sha256,
            confirm_reset: true,
        })
    }

    pub fn export_html(&self, output: impl AsRef<Path>, top_folder: &str) -> Result<PathBuf> {
        let conn = self.connection()?;
        let (html, _, _) = render_bookmarks_html(&conn, top_folder)?;
        let output = output.as_ref();
        write_atomic(output, html.as_bytes())?;
        Ok(output.to_path_buf())
    }

    pub fn save_snapshot(&self, input: SaveSnapshotInput) -> Result<SnapshotResult> {
        if !matches!(input.kind.as_str(), "semantic" | "verbatim") {
            return Err(VaultError::InvalidInput(
                "snapshot kind must be semantic or verbatim".to_owned(),
            ));
        }
        if input.content.contains('\0') {
            return Err(VaultError::InvalidInput(
                "snapshot content contains NUL".to_owned(),
            ));
        }
        if let Some(captured_at) = input.captured_at.as_deref() {
            DateTime::parse_from_rfc3339(&captured_at.replace('Z', "+00:00"))
                .map_err(|_| VaultError::InvalidInput("invalid captured_at".to_owned()))?;
        }
        let current = self.get_url(&input.target)?;
        let raw = input.content.as_bytes();
        let sha256 = hex_sha256(raw);
        let relative = PathBuf::from("snapshots")
            .join(&current.id)
            .join(format!("{sha256}.md"));
        let artifact = self.resolve_artifact_for_write(&relative)?;
        if artifact.exists() {
            if !artifact.is_file() || fs::read(&artifact)? != raw {
                return Err(VaultError::InvalidInput(format!(
                    "content-addressed snapshot collision at {}",
                    artifact.display()
                )));
            }
        } else {
            write_atomic(&artifact, raw)?;
        }
        let metadata = SnapshotMetadata {
            artifact_path: relative.to_string_lossy().to_string(),
            content_sha256: sha256,
            kind: input.kind,
            title: input.title.or(current.title.clone()),
            captured_at: Some(input.captured_at.unwrap_or_else(now_iso)),
            byte_count: Some(i64::try_from(raw.len()).unwrap_or(i64::MAX)),
        };
        let previous = current.snapshot.as_ref().map(|value| value.sha256.clone());
        let result = self.mutate(|transaction| {
            let snapshot =
                upsert_snapshot_reference(transaction, &self.home, &current.id, &metadata)?;
            transaction.execute(
                "UPDATE bookmarks SET updated_at = ?1 WHERE id = ?2",
                params![now_iso(), current.id],
            )?;
            Ok(snapshot)
        })?;
        Ok(SnapshotResult {
            bookmark: BookmarkIdentity {
                id: current.id,
                title: current.title,
                url: current.url,
            },
            snapshot: result.item,
            canonical_html: result.canonical_html,
            replaced_snapshot: previous,
        })
    }

    pub fn get_snapshot(&self, target: &str) -> Result<SnapshotPayload> {
        let bookmark = self.get_url(target)?;
        let snapshot = self.verify_one_snapshot(&bookmark.id, true)?;
        Ok(SnapshotPayload {
            bookmark: BookmarkIdentity {
                id: bookmark.id,
                title: bookmark.title,
                url: bookmark.url,
            },
            snapshot,
        })
    }

    pub fn verify_snapshots(&self, target: Option<&str>) -> Result<SnapshotVerification> {
        let bookmarks = if let Some(target) = target {
            vec![self.get_url(target)?]
        } else {
            self.query_urls(None, None)?
                .into_iter()
                .filter(|bookmark| bookmark.snapshot.is_some())
                .collect()
        };
        let mut results = Vec::new();
        let mut valid = 0;
        for bookmark in bookmarks {
            let snapshot = self.verify_one_snapshot(&bookmark.id, false)?;
            if snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.status == "valid")
            {
                valid += 1;
            }
            results.push(SnapshotPayload {
                bookmark: BookmarkIdentity {
                    id: bookmark.id,
                    title: bookmark.title,
                    url: bookmark.url,
                },
                snapshot,
            });
        }
        Ok(SnapshotVerification {
            checked: results.len(),
            valid,
            invalid: results.len() - valid,
            results,
        })
    }

    pub fn open_url(
        &self,
        target: &str,
        browser: Option<&str>,
        dry_run: bool,
        opened_by: &str,
        context: Option<&str>,
    ) -> Result<OpenResult> {
        let (bookmark, url) = if target.starts_with("http://") || target.starts_with("https://") {
            let bookmark = self.get_url(target).ok();
            (bookmark, target.to_owned())
        } else if let Ok(bookmark) = self
            .connection()
            .and_then(|connection| load_bookmark(&connection, target))
        {
            let url = bookmark.url.clone();
            (Some(bookmark), url)
        } else {
            let candidates = self.search_urls(target, 5)?;
            if !search::is_clear_winner(&candidates) {
                return Err(VaultError::Ambiguous(format!(
                    "no clear URL winner for {target}"
                )));
            }
            let bookmark = candidates.into_iter().next().ok_or_else(|| {
                VaultError::NotFound(format!("no matching URL found for {target}"))
            })?;
            let url = bookmark.url.clone();
            (Some(bookmark), url)
        };
        let parsed = url::Url::parse(&url)?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(VaultError::InvalidInput(
                "only http and https URLs can be opened".to_owned(),
            ));
        }
        let selected = normalize_browser_policy(
            browser.or_else(|| bookmark.as_ref()?.preferred_browser.as_deref()),
        );
        if !dry_run {
            launch_url(&url, &selected)?;
        }
        self.record_open(
            bookmark.as_ref().map(|value| value.id.as_str()),
            &url,
            &selected,
            opened_by,
            context,
        )?;
        Ok(OpenResult {
            url,
            browser: selected,
            bookmark,
            dry_run,
        })
    }

    pub fn show_vault(app_path: Option<&Path>) -> Result<PathBuf> {
        let app = app_path
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("CODEX_URL_VAULT_APP_PATH").map(PathBuf::from))
            .unwrap_or_else(|| expand_home(DEFAULT_APP_PATH));
        if !app.exists() {
            return Err(VaultError::NotFound(format!(
                "native app not found at {}",
                app.display()
            )));
        }
        let status = Command::new("/usr/bin/open").arg(&app).status()?;
        if !status.success() {
            return Err(VaultError::InvalidInput(format!(
                "open failed for {}",
                app.display()
            )));
        }
        Ok(app)
    }

    fn connection(&self) -> Result<Connection> {
        let conn = self.raw_connection()?;
        let pending = meta_value(&conn, "export_pending")? != "0";
        if pending {
            drop(conn);
            let _guard = self.mutation_guard()?;
            let conn = self.raw_connection()?;
            self.recover_export_locked(&conn)?;
            return Ok(conn);
        }
        Ok(conn)
    }

    fn raw_connection(&self) -> Result<Connection> {
        fs::create_dir_all(&self.home)?;
        let conn = Connection::open(self.db_path())?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        schema::ensure_schema(&conn)?;
        Ok(conn)
    }

    fn mutation_guard(&self) -> Result<MutationGuard> {
        fs::create_dir_all(&self.home)?;
        let path = self.home.join(".mutation.lock");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;
        file.lock_exclusive()?;
        Ok(MutationGuard { file })
    }

    fn mutate<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T>,
    ) -> Result<MutationResult<T>> {
        let _guard = self.mutation_guard()?;
        let mut conn = self.raw_connection()?;
        self.recover_export_locked(&conn)?;
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let item = operation(&transaction)?;
        let canonical_html = self.commit_transaction(transaction)?;
        Ok(MutationResult {
            item,
            canonical_html,
        })
    }

    fn commit_transaction(&self, transaction: Transaction<'_>) -> Result<String> {
        let generation = meta_value(&transaction, "export_generation")?
            .parse::<u64>()
            .unwrap_or(0)
            + 1;
        transaction.execute(
            "UPDATE vault_meta SET value = ?1 WHERE key = 'export_generation'",
            [generation.to_string()],
        )?;
        transaction.execute(
            "UPDATE vault_meta SET value = '1' WHERE key = 'export_pending'",
            [],
        )?;
        let (html, _, _) = render_bookmarks_html(&transaction, "Codex URL Vault")?;
        let staged = self.stage_canonical_html(html.as_bytes())?;
        if let Err(error) = transaction.commit() {
            let _ = fs::remove_file(&staged);
            return Err(error.into());
        }
        if let Err(error) = replace_staged(&staged, &self.canonical_html_path()) {
            return Err(error);
        }
        let conn = self.raw_connection()?;
        conn.execute(
            "UPDATE vault_meta SET value = '0' WHERE key = 'export_pending'",
            [],
        )?;
        Ok(self.canonical_html_path().display().to_string())
    }

    fn recover_export_locked(&self, conn: &Connection) -> Result<()> {
        self.remove_stale_export_temps()?;
        if meta_value(conn, "export_pending")? == "0" {
            return Ok(());
        }
        let (html, _, _) = render_bookmarks_html(conn, "Codex URL Vault")?;
        write_atomic(&self.canonical_html_path(), html.as_bytes())?;
        conn.execute(
            "UPDATE vault_meta SET value = '0' WHERE key = 'export_pending'",
            [],
        )?;
        Ok(())
    }

    fn stage_canonical_html(&self, bytes: &[u8]) -> Result<PathBuf> {
        fs::create_dir_all(&self.home)?;
        let staged = self.home.join(format!(
            ".current-bookmarks.html.tmp-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(staged)
    }

    fn remove_stale_export_temps(&self) -> Result<()> {
        let Ok(entries) = fs::read_dir(&self.home) else {
            return Ok(());
        };
        for entry in entries {
            let entry = entry?;
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(".current-bookmarks.html.tmp-")
            {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }

    fn resolve_bookmark(&self, conn: &Connection, target: &str, limit: usize) -> Result<Bookmark> {
        resolve_bookmark_on(conn, target, limit)
    }

    fn list_urls_with_snapshot_content(&self) -> Result<Vec<Bookmark>> {
        let bookmarks = self.query_urls(None, None)?;
        self.verify_bookmark_snapshots(bookmarks, true)
    }

    fn verify_bookmark_snapshots(
        &self,
        mut bookmarks: Vec<Bookmark>,
        include_content: bool,
    ) -> Result<Vec<Bookmark>> {
        for bookmark in &mut bookmarks {
            if bookmark.snapshot.is_some() {
                bookmark.snapshot = self.verify_one_snapshot(&bookmark.id, include_content)?;
            }
        }
        Ok(bookmarks)
    }

    fn backup_database(&self, conn: &Connection, reason: &str) -> Result<String> {
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S-%6f");
        let path = self
            .db_path()
            .with_file_name(format!("vault.sqlite.bak-{reason}-{timestamp}"));
        conn.execute("VACUUM INTO ?1", [path.display().to_string()])?;
        Ok(path.display().to_string())
    }

    fn resolve_artifact_for_write(&self, relative: &Path) -> Result<PathBuf> {
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(VaultError::UnsafePath(relative.to_path_buf()));
        }
        let parent = self
            .home
            .join(relative)
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| VaultError::UnsafePath(relative.to_path_buf()))?;
        fs::create_dir_all(&parent)?;
        let root = fs::canonicalize(&self.home)?;
        let parent = fs::canonicalize(&parent)?;
        if !parent.starts_with(&root) {
            return Err(VaultError::UnsafePath(relative.to_path_buf()));
        }
        Ok(parent.join(
            relative
                .file_name()
                .ok_or_else(|| VaultError::UnsafePath(relative.to_path_buf()))?,
        ))
    }

    fn verify_one_snapshot(
        &self,
        bookmark_id: &str,
        include_content: bool,
    ) -> Result<Option<Snapshot>> {
        let _guard = self.mutation_guard()?;
        let conn = self.raw_connection()?;
        self.recover_export_locked(&conn)?;
        let metadata = conn
            .query_row(
                r#"
                SELECT artifact_path, content_sha256, kind, title, captured_at, byte_count
                  FROM snapshots WHERE bookmark_id = ?1
                "#,
                [bookmark_id],
                |row| {
                    Ok(SnapshotMetadata {
                        artifact_path: row.get(0)?,
                        content_sha256: row.get(1)?,
                        kind: row.get(2)?,
                        title: row.get(3)?,
                        captured_at: row.get(4)?,
                        byte_count: row.get(5)?,
                    })
                },
            )
            .optional()?;
        let Some(metadata) = metadata else {
            return Ok(None);
        };
        let snapshot = inspect_snapshot(&self.home, bookmark_id, &metadata, include_content)?;
        conn.execute(
            r#"
            UPDATE snapshots
               SET content_text = ?1, status = ?2, verified_at = ?3, updated_at = ?3
             WHERE bookmark_id = ?4
            "#,
            params![
                snapshot.content,
                snapshot.status,
                snapshot.verified_at,
                bookmark_id
            ],
        )?;
        Ok(Some(snapshot))
    }

    fn record_open(
        &self,
        bookmark_id: Option<&str>,
        url: &str,
        browser: &str,
        opened_by: &str,
        context: Option<&str>,
    ) -> Result<()> {
        let _guard = self.mutation_guard()?;
        let conn = self.raw_connection()?;
        let opened_at = now_iso();
        let open_id = short_hash(format!("{}{}{}{}", url, browser, opened_by, opened_at));
        conn.execute(
            r#"
            INSERT INTO opens(id, bookmark_id, url, opened_browser, opened_by, opened_at, context)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                open_id,
                bookmark_id,
                url,
                browser,
                opened_by,
                opened_at,
                context
            ],
        )?;
        if let Some(bookmark_id) = bookmark_id {
            conn.execute(
                r#"
                UPDATE bookmarks
                   SET last_opened_at = ?1,
                       open_count = open_count + 1,
                       preferred_browser = CASE WHEN ?2 = 'default' THEN preferred_browser ELSE ?2 END,
                       updated_at = ?1
                 WHERE id = ?3
                "#,
                params![opened_at, browser, bookmark_id],
            )?;
        }
        Ok(())
    }
}

struct MutationGuard {
    file: File,
}

impl Drop for MutationGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn bookmark_from_row(row: &Row<'_>) -> rusqlite::Result<Bookmark> {
    bookmark_from_row_impl(row, false)
}

fn bookmark_from_row_with_content(row: &Row<'_>) -> rusqlite::Result<Bookmark> {
    bookmark_from_row_impl(row, true)
}

fn bookmark_from_row_impl(
    row: &Row<'_>,
    include_snapshot_content: bool,
) -> rusqlite::Result<Bookmark> {
    let snapshot_id = row.get::<_, Option<String>>("snapshot_bookmark_id")?;
    let snapshot = if let Some(snapshot_id) = snapshot_id {
        let content = row.get::<_, Option<String>>("snapshot_content_text")?;
        let status = row.get::<_, String>("snapshot_status")?;
        Some(Snapshot {
            bookmark_id: snapshot_id,
            artifact_path: row.get("snapshot_artifact_path")?,
            sha256: row.get("snapshot_sha256")?,
            kind: row.get("snapshot_kind")?,
            title: row.get("snapshot_title")?,
            captured_at: row.get("snapshot_captured_at")?,
            byte_count: row.get("snapshot_byte_count")?,
            status: status.clone(),
            verified_at: row.get("snapshot_verified_at")?,
            updated_at: row.get("snapshot_updated_at")?,
            preview: if status == "valid" {
                content
                    .as_deref()
                    .map(|value| value.chars().take(320).collect())
            } else {
                None
            },
            content: (status == "valid" && include_snapshot_content)
                .then_some(content)
                .flatten(),
        })
    } else {
        None
    };
    Ok(Bookmark {
        id: row.get("id")?,
        url: row.get("url")?,
        canonical_url: row.get("canonical_url")?,
        title: row.get("title")?,
        description: row.get("description")?,
        note: row.get("note")?,
        tags: parse_json_list(row.get("tags_json")?),
        aliases: parse_json_list(row.get("aliases_json")?),
        intents: parse_json_list(row.get("intents_json")?),
        source_type: row.get("source_type")?,
        source_browser: row.get("source_browser")?,
        source_profile: row.get("source_profile")?,
        folder_path: row.get("folder_path")?,
        preferred_browser: row.get("preferred_browser")?,
        project: row.get("project")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        first_seen_at: row.get("first_seen_at")?,
        last_seen_at: row.get("last_seen_at")?,
        last_opened_at: row.get("last_opened_at")?,
        open_count: row.get("open_count")?,
        seen_in_latest_import: row.get::<_, i64>("seen_in_latest_import")? != 0,
        missing_count: row.get("missing_count")?,
        snapshot,
        score: None,
    })
}

fn load_bookmark(conn: &Connection, bookmark_id: &str) -> Result<Bookmark> {
    let sql = format!("{BOOKMARK_SELECT} WHERE b.id = ?1 AND b.status = 'active'");
    conn.query_row(&sql, [bookmark_id], bookmark_from_row)
        .optional()?
        .ok_or_else(|| VaultError::NotFound(format!("bookmark {bookmark_id}")))
}

fn load_bookmark_any_status(conn: &Connection, bookmark_id: &str) -> Result<Bookmark> {
    let sql = format!("{BOOKMARK_SELECT} WHERE b.id = ?1");
    conn.query_row(&sql, [bookmark_id], bookmark_from_row)
        .optional()?
        .ok_or_else(|| VaultError::NotFound(format!("bookmark {bookmark_id}")))
}

fn resolve_bookmark_on(conn: &Connection, target: &str, limit: usize) -> Result<Bookmark> {
    if let Ok(bookmark) = load_bookmark(conn, target) {
        return Ok(bookmark);
    }
    if target.starts_with("http://") || target.starts_with("https://") {
        let canonical = canonicalize_url(target);
        let sql = format!("{BOOKMARK_SELECT} WHERE b.canonical_url = ?1 AND b.status = 'active'");
        if let Some(bookmark) = conn
            .query_row(&sql, [&canonical], bookmark_from_row)
            .optional()?
        {
            return Ok(bookmark);
        }
    }
    let sql = format!("{BOOKMARK_SELECT} WHERE b.status = 'active'");
    let mut statement = conn.prepare(&sql)?;
    let bookmarks = statement
        .query_map([], bookmark_from_row_with_content)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    search::search(bookmarks, target, limit)
        .into_iter()
        .next()
        .map(redact_snapshot_content)
        .ok_or_else(|| VaultError::NotFound(format!("no matching bookmark found: {target}")))
}

fn redact_snapshot_content(mut bookmark: Bookmark) -> Bookmark {
    if let Some(snapshot) = &mut bookmark.snapshot {
        snapshot.content = None;
    }
    bookmark
}

fn upsert_bookmark(conn: &Connection, input: &SaveUrlInput) -> Result<Bookmark> {
    let canonical = canonicalize_url(&input.url);
    let id = bookmark_id(&canonical);
    let timestamp = now_iso();
    let existing = conn
        .query_row(
            "SELECT tags_json, aliases_json, intents_json FROM bookmarks WHERE canonical_url = ?1",
            [&canonical],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let folder = input
        .folder_path
        .as_deref()
        .map(|value| normalize_category_path(Some(value)));
    let preferred = input
        .preferred_browser
        .as_deref()
        .map(|value| normalize_browser_policy(Some(value)));
    if let Some((tags, aliases, intents)) = existing {
        conn.execute(
            r#"
            UPDATE bookmarks
               SET url = COALESCE(?1, url),
                   title = COALESCE(NULLIF(?2, ''), title),
                   description = COALESCE(NULLIF(?3, ''), description),
                   note = COALESCE(NULLIF(?4, ''), note),
                   tags_json = ?5,
                   aliases_json = ?6,
                   intents_json = ?7,
                   source_type = COALESCE(NULLIF(?8, ''), source_type),
                   source_browser = COALESCE(NULLIF(?9, ''), source_browser),
                   source_profile = COALESCE(NULLIF(?10, ''), source_profile),
                   folder_path = COALESCE(NULLIF(?11, ''), folder_path),
                   preferred_browser = COALESCE(NULLIF(?12, ''), preferred_browser),
                   project = COALESCE(NULLIF(?13, ''), project),
                   status = COALESCE(NULLIF(?14, ''), status),
                   updated_at = ?15,
                   last_seen_at = COALESCE(?16, last_seen_at),
                   seen_in_latest_import = 1,
                   missing_count = 0
             WHERE canonical_url = ?17
            "#,
            params![
                input.url,
                input.title,
                input.description,
                input.note,
                merge_json_list(&tags, &input.tags),
                merge_json_list(&aliases, &input.aliases),
                merge_json_list(&intents, &input.intents),
                input.source_type,
                input.source_browser,
                input.source_profile,
                folder,
                preferred,
                input.project,
                input.status,
                timestamp,
                timestamp,
                canonical
            ],
        )?;
    } else {
        conn.execute(
            r#"
            INSERT INTO bookmarks(
                id, url, canonical_url, title, description, note, tags_json, aliases_json,
                intents_json, source_type, source_browser, source_profile, folder_path,
                preferred_browser, project, status, created_at, updated_at, first_seen_at,
                last_seen_at, seen_in_latest_import
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?17, ?17, ?17, 1
            )
            "#,
            params![
                id,
                input.url,
                canonical,
                input.title,
                input.description,
                input.note,
                serde_json::to_string(&deduplicate(&input.tags))?,
                serde_json::to_string(&deduplicate(&input.aliases))?,
                serde_json::to_string(&deduplicate(&input.intents))?,
                input.source_type.as_deref().unwrap_or("manual"),
                input.source_browser,
                input.source_profile,
                folder,
                preferred,
                input.project,
                input.status.as_deref().unwrap_or("active"),
                timestamp
            ],
        )?;
    }
    load_bookmark(conn, &id)
}

fn upsert_category(
    conn: &Connection,
    path: &str,
    label: Option<&str>,
    note: Option<&str>,
) -> Result<()> {
    let path = normalize_category_path(Some(path));
    let timestamp = now_iso();
    conn.execute(
        r#"
        INSERT INTO categories(path, label, note, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, 'active', ?4, ?4)
        ON CONFLICT(path) DO UPDATE SET
          label = COALESCE(NULLIF(excluded.label, ''), categories.label),
          note = COALESCE(NULLIF(excluded.note, ''), categories.note),
          status = 'active',
          updated_at = excluded.updated_at
        "#,
        params![
            path,
            label.unwrap_or(&category_display(&path)),
            note,
            timestamp
        ],
    )?;
    Ok(())
}

fn load_category(conn: &Connection, path: &str) -> Result<Category> {
    load_category_with_status(conn, path, true)
}

fn load_category_any_status(conn: &Connection, path: &str) -> Result<Category> {
    load_category_with_status(conn, path, false)
}

fn load_category_with_status(conn: &Connection, path: &str, active_only: bool) -> Result<Category> {
    let mut sql =
        "SELECT path, label, note, status, created_at, updated_at FROM categories WHERE path = ?1"
            .to_owned();
    if active_only {
        sql.push_str(" AND status = 'active'");
    }
    conn.query_row(&sql, [path], |row| {
        let path = row.get::<_, String>(0)?;
        Ok(Category {
            display: category_display(&path),
            label: row
                .get::<_, Option<String>>(1)?
                .unwrap_or_else(|| category_display(&path)),
            note: row.get(2)?,
            status: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
            count: 0,
            path,
        })
    })
    .optional()?
    .ok_or_else(|| VaultError::NotFound(format!("category {path}")))
}

fn upsert_snapshot_reference(
    conn: &Connection,
    home: &Path,
    bookmark_id: &str,
    metadata: &SnapshotMetadata,
) -> Result<Snapshot> {
    let snapshot = inspect_snapshot(home, bookmark_id, metadata, true)?;
    let timestamp = now_iso();
    conn.execute(
        r#"
        INSERT INTO snapshots(
          bookmark_id, artifact_path, content_sha256, kind, title, captured_at,
          byte_count, content_text, status, verified_at, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
        ON CONFLICT(bookmark_id) DO UPDATE SET
          artifact_path = excluded.artifact_path,
          content_sha256 = excluded.content_sha256,
          kind = excluded.kind,
          title = excluded.title,
          captured_at = excluded.captured_at,
          byte_count = excluded.byte_count,
          content_text = excluded.content_text,
          status = excluded.status,
          verified_at = excluded.verified_at,
          updated_at = excluded.updated_at
        "#,
        params![
            bookmark_id,
            metadata.artifact_path,
            metadata.content_sha256,
            metadata.kind,
            metadata.title,
            metadata.captured_at,
            metadata.byte_count,
            snapshot.content,
            snapshot.status,
            snapshot.verified_at,
            timestamp
        ],
    )?;
    Ok(snapshot)
}

fn inspect_snapshot(
    home: &Path,
    bookmark_id: &str,
    metadata: &SnapshotMetadata,
    include_content: bool,
) -> Result<Snapshot> {
    let verified_at = now_iso();
    let mut status = "valid".to_owned();
    let mut content = None;
    if metadata.artifact_path.is_empty()
        || metadata.content_sha256.len() != 64
        || !metadata
            .content_sha256
            .chars()
            .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
        || !matches!(metadata.kind.as_str(), "semantic" | "verbatim")
        || metadata.captured_at.is_none()
        || metadata.byte_count.is_none_or(|value| value < 0)
    {
        status = "invalid_metadata".to_owned();
    } else {
        let relative = Path::new(&metadata.artifact_path);
        let safe_relative = !relative.is_absolute()
            && !relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            });
        if !safe_relative {
            status = "unsafe_path".to_owned();
        } else {
            let root = fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
            let path = home.join(relative);
            let resolved = if path.exists() {
                fs::canonicalize(&path).ok()
            } else {
                path.parent()
                    .and_then(|parent| fs::canonicalize(parent).ok())
                    .and_then(|parent| path.file_name().map(|name| parent.join(name)))
            };
            if resolved
                .as_ref()
                .is_none_or(|path| !path.starts_with(&root))
            {
                status = "unsafe_path".to_owned();
            } else if !path.is_file() {
                status = "missing".to_owned();
            } else {
                match fs::read(&path) {
                    Err(_) => status = "unreadable".to_owned(),
                    Ok(raw) if hex_sha256(&raw) != metadata.content_sha256 => {
                        status = "hash_mismatch".to_owned();
                    }
                    Ok(raw) if Some(raw.len() as i64) != metadata.byte_count => {
                        status = "byte_mismatch".to_owned();
                    }
                    Ok(raw) => match String::from_utf8(raw) {
                        Err(_) => status = "invalid_utf8".to_owned(),
                        Ok(value) => content = Some(value),
                    },
                }
            }
        }
    }
    let preview = if status == "valid" {
        content
            .as_deref()
            .map(|value| value.chars().take(320).collect())
    } else {
        None
    };
    Ok(Snapshot {
        bookmark_id: bookmark_id.to_owned(),
        artifact_path: metadata.artifact_path.clone(),
        sha256: metadata.content_sha256.clone(),
        kind: metadata.kind.clone(),
        title: metadata.title.clone(),
        captured_at: metadata.captured_at.clone(),
        byte_count: metadata.byte_count,
        status,
        verified_at: Some(verified_at.clone()),
        updated_at: verified_at,
        preview,
        content: include_content.then_some(content).flatten(),
    })
}

fn import_item_to_save_input(
    item: &crate::model::ImportItem,
    source_browser: &str,
    request: &ApplyImportInput,
) -> SaveUrlInput {
    SaveUrlInput {
        url: item.url.clone(),
        title: Some(item.title.clone()),
        description: item.description.clone(),
        note: item.note.clone(),
        tags: item.tags.clone(),
        aliases: item.aliases.clone(),
        intents: item.intents.clone(),
        source_type: Some("browser_import".to_owned()),
        source_browser: Some(source_browser.to_owned()),
        source_profile: Some(request.source_profile.trim().to_owned()),
        folder_path: Some(item.folder_path.clone()),
        preferred_browser: request.preferred_browser.clone(),
        project: None,
        status: Some("active".to_owned()),
    }
}

fn reset_vault_data(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        DELETE FROM opens;
        DELETE FROM bookmark_imports;
        DELETE FROM imports;
        DELETE FROM snapshots;
        DELETE FROM bookmarks;
        DELETE FROM categories;
        "#,
    )?;
    Ok(())
}

fn set_json_list(conn: &Connection, id: &str, column: &str, values: &[String]) -> Result<()> {
    if !matches!(column, "tags_json" | "aliases_json" | "intents_json") {
        return Err(VaultError::InvalidInput(
            "invalid JSON list column".to_owned(),
        ));
    }
    conn.execute(
        &format!("UPDATE bookmarks SET {column} = ?1, updated_at = ?2 WHERE id = ?3"),
        params![serde_json::to_string(&deduplicate(values))?, now_iso(), id],
    )?;
    Ok(())
}

fn parse_json_list(value: String) -> Vec<String> {
    serde_json::from_str(&value).unwrap_or_default()
}

fn merge_json_list(existing: &str, incoming: &[String]) -> String {
    let mut values = parse_json_list(existing.to_owned());
    for value in incoming {
        if !value.is_empty() && !values.contains(value) {
            values.push(value.clone());
        }
    }
    serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_owned())
}

fn deduplicate(values: &[String]) -> Vec<String> {
    let mut output = Vec::new();
    for value in values {
        if !value.is_empty() && !output.contains(value) {
            output.push(value.clone());
        }
    }
    output
}

pub fn canonicalize_url(value: &str) -> String {
    let mut value = value
        .trim()
        .split('#')
        .next()
        .unwrap_or_default()
        .to_owned();
    let has_scheme = value.find(':').is_some_and(|colon| {
        value[..colon]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    if !has_scheme {
        value = format!("https://{value}");
    }
    let Some(colon) = value.find(':') else {
        return value;
    };
    let scheme = value[..colon].to_lowercase();
    let mut remainder = &value[colon + 1..];
    if let Some(authority_and_path) = remainder.strip_prefix("//") {
        let (authority, path_and_query) = authority_and_path
            .find(['/', '?'])
            .map(|index| (&authority_and_path[..index], &authority_and_path[index..]))
            .unwrap_or((authority_and_path, "/"));
        let (path, query) = if let Some(query) = path_and_query.strip_prefix('?') {
            ("/", Some(query))
        } else {
            path_and_query
                .split_once('?')
                .map(|(path, query)| (path, Some(query)))
                .unwrap_or((path_and_query, None))
        };
        let mut path = if path.is_empty() { "/" } else { path }.to_owned();
        if path != "/" {
            while path.ends_with('/') {
                path.pop();
            }
        }
        let query = query.map(|query| format!("?{query}")).unwrap_or_default();
        format!("{scheme}://{}{path}{query}", authority.to_lowercase())
    } else {
        if remainder.is_empty() {
            remainder = "/";
        }
        format!("{scheme}:{remainder}")
    }
}

pub fn bookmark_id(canonical_url: &str) -> String {
    hex_sha256(canonical_url.as_bytes())[..24].to_owned()
}

pub fn normalize_category_path(value: Option<&str>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let normalized = value
        .replace('\\', "/")
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if matches!(
        normalized.to_lowercase().as_str(),
        "unfiled" | "(unfiled)" | "none" | "root"
    ) {
        String::new()
    } else {
        normalized
    }
}

fn category_display(path: &str) -> String {
    if path.is_empty() {
        "Unfiled".to_owned()
    } else {
        path.to_owned()
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn short_hash(value: String) -> String {
    hex_sha256(value.as_bytes())[..24].to_owned()
}

fn meta_value(conn: &Connection, key: &str) -> Result<String> {
    conn.query_row(
        "SELECT value FROM vault_meta WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn normalize_browser_policy(value: Option<&str>) -> String {
    match value.unwrap_or("default").trim().to_lowercase().as_str() {
        "" | "default" => "default".to_owned(),
        "brave" => "brave".to_owned(),
        "chrome" => "chrome".to_owned(),
        "firefox" => "firefox".to_owned(),
        "safari" => "safari".to_owned(),
        _ => "default".to_owned(),
    }
}

fn launch_url(url: &str, browser: &str) -> Result<()> {
    let mut command = Command::new("/usr/bin/open");
    match browser {
        "default" => {
            command.arg(url);
        }
        "brave" => {
            command.args(["-a", "Brave Browser", url]);
        }
        "chrome" => {
            command.args(["-a", "Google Chrome", url]);
        }
        "firefox" => {
            command.args(["-a", "Firefox", url]);
        }
        "safari" => {
            command.args(["-a", "Safari", url]);
        }
        _ => {
            return Err(VaultError::InvalidInput(format!(
                "unsupported browser: {browser}"
            )));
        }
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(VaultError::InvalidInput(format!(
            "open failed for browser {browser}"
        )))
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        VaultError::InvalidInput(format!("path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent)?;
    let staged = parent.join(format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("vault"),
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    replace_staged(&staged, path)
}

fn replace_staged(staged: &Path, destination: &Path) -> Result<()> {
    fs::rename(staged, destination)?;
    if let Some(parent) = destination.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn expand_home(value: &str) -> PathBuf {
    expand_home_from(value, std::env::var_os("HOME").map(PathBuf::from))
}

fn expand_home_from(value: &str, home: Option<PathBuf>) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/")
        && let Some(home) = home
    {
        return home.join(rest);
    }
    PathBuf::from(value)
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn default_app_path_is_resolved_from_the_current_home() {
        assert_eq!(
            expand_home_from(DEFAULT_APP_PATH, Some(PathBuf::from("/Users/example"))),
            PathBuf::from("/Users/example/Applications/Codex URL Vault.app")
        );
    }
}
