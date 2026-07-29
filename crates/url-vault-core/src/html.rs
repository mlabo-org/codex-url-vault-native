use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer};
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::error::{Result, VaultError};
use crate::model::{ImportItem, ImportPreview, ParsedImport, SnapshotMetadata};
use crate::store::{canonicalize_url, normalize_category_path};

const MAX_BOOKMARK_HTML_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone)]
enum CaptureKind {
    Folder,
    Link,
}

#[derive(Debug, Clone)]
struct Capture {
    kind: CaptureKind,
    text: String,
    attrs: HashMap<String, String>,
}

#[derive(Debug, Default)]
struct ParserState {
    folder_stack: Vec<String>,
    pending_folder: Option<String>,
    capture: Option<Capture>,
    items: Vec<ImportItem>,
    parse_error: Option<String>,
}

#[derive(Debug, Default)]
struct BookmarkSink(RefCell<ParserState>);

impl TokenSink for BookmarkSink {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<Self::Handle> {
        let mut state = self.0.borrow_mut();
        match token {
            Token::TagToken(tag) => {
                let name = tag.name.as_ref();
                match (tag.kind, name) {
                    (TagKind::StartTag, "h3") => {
                        state.capture = Some(Capture {
                            kind: CaptureKind::Folder,
                            text: String::new(),
                            attrs: attributes(&tag.attrs),
                        });
                    }
                    (TagKind::StartTag, "a") => {
                        state.capture = Some(Capture {
                            kind: CaptureKind::Link,
                            text: String::new(),
                            attrs: attributes(&tag.attrs),
                        });
                    }
                    (TagKind::StartTag, "dl") => {
                        if let Some(folder) = state.pending_folder.take() {
                            state.folder_stack.push(folder);
                        }
                    }
                    (TagKind::EndTag, "h3") => {
                        if let Some(capture) = state.capture.take()
                            && matches!(capture.kind, CaptureKind::Folder)
                        {
                            let folder = clean_text(&capture.text);
                            if !folder.is_empty() {
                                state.pending_folder = Some(folder);
                            }
                        }
                    }
                    (TagKind::EndTag, "a") => {
                        if let Some(capture) = state.capture.take()
                            && matches!(capture.kind, CaptureKind::Link)
                            && let Some(href) = capture.attrs.get("href").cloned()
                        {
                            let title = {
                                let cleaned = clean_text(&capture.text);
                                if cleaned.is_empty() {
                                    href.clone()
                                } else {
                                    cleaned
                                }
                            };
                            let snapshot = snapshot_from_attrs(&capture.attrs);
                            let folder_path = state.folder_stack.join("/");
                            state.items.push(ImportItem {
                                url: href,
                                title,
                                folder_path,
                                add_date: capture.attrs.get("add_date").cloned(),
                                last_modified: capture.attrs.get("last_modified").cloned(),
                                tags: parse_list(capture.attrs.get("tags").map(String::as_str)),
                                aliases: parse_list(
                                    capture.attrs.get("aliases").map(String::as_str),
                                ),
                                intents: parse_list(
                                    capture.attrs.get("intents").map(String::as_str),
                                ),
                                note: capture.attrs.get("note").cloned(),
                                description: capture.attrs.get("description").cloned(),
                                snapshot,
                            });
                        }
                    }
                    (TagKind::EndTag, "dl") => {
                        state.folder_stack.pop();
                    }
                    _ => {}
                }
            }
            Token::CharacterTokens(value) => {
                if let Some(capture) = &mut state.capture {
                    capture.text.push_str(value.as_ref());
                }
            }
            Token::NullCharacterToken => {
                state.parse_error = Some("bookmark_html_contains_nul".to_owned());
            }
            Token::ParseError(error) => {
                if state.parse_error.is_none() {
                    state.parse_error = Some(format!("bookmark_html_invalid: {error}"));
                }
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

fn attributes(values: &[html5ever::Attribute]) -> HashMap<String, String> {
    values
        .iter()
        .map(|attribute| {
            (
                attribute.name.local.as_ref().to_lowercase(),
                attribute.value.to_string(),
            )
        })
        .collect()
}

pub(crate) fn parse_bookmark_html(
    text: &str,
    file_name: &str,
    strip_root: bool,
) -> Result<ParsedImport> {
    if text.len() > MAX_BOOKMARK_HTML_BYTES {
        return Err(VaultError::InvalidInput(
            "bookmark_html_exceeds_20_mib".to_owned(),
        ));
    }
    if text.contains('\0') {
        return Err(VaultError::InvalidInput(
            "bookmark_html_contains_nul".to_owned(),
        ));
    }
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<dl") && !lower.contains("<!doctype netscape-bookmark-file-1") {
        return Err(VaultError::InvalidInput(
            "bookmark_html_unrecognized".to_owned(),
        ));
    }

    let input = BufferQueue::default();
    input.push_back(StrTendril::from(text));
    let tokenizer = Tokenizer::new(BookmarkSink::default(), Default::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let mut state = tokenizer.sink.0.into_inner();
    if let Some(error) = state.parse_error {
        return Err(VaultError::InvalidInput(error));
    }
    if state.items.is_empty() {
        return Err(VaultError::InvalidInput(
            "bookmark_html_has_no_bookmarks".to_owned(),
        ));
    }

    let common_root = common_root(&state.items);
    let stripped_root = if strip_root {
        strip_common_root(&mut state.items)
    } else {
        None
    };
    let parsed_count = state.items.len();
    let mut seen = HashSet::new();
    state
        .items
        .retain(|item| seen.insert(canonicalize_url(&item.url)));
    let unique_count = state.items.len();
    let mut folders = state
        .items
        .iter()
        .map(|item| normalize_category_path(Some(&item.folder_path)))
        .collect::<Vec<_>>();
    folders.sort_by_key(|value| value.to_lowercase());
    folders.dedup();
    let safe_file_name = sanitize_file_name(file_name);
    let content_sha256 = hex_sha256(text.as_bytes());

    Ok(ParsedImport {
        preview: ImportPreview {
            file_name: safe_file_name,
            content_sha256,
            parsed_count,
            unique_count,
            duplicate_count: parsed_count - unique_count,
            folder_count: folders.len(),
            folders: folders.into_iter().take(12).collect(),
            common_root,
            stripped_root,
        },
        items: state.items,
    })
}

pub(crate) fn render_bookmarks_html(
    conn: &Connection,
    top_folder: &str,
) -> Result<(String, usize, usize)> {
    let mut rows = conn.prepare(
        r#"
        SELECT id, url, title, description, note, tags_json, aliases_json, intents_json,
               folder_path, created_at, updated_at, first_seen_at
          FROM bookmarks
         WHERE status = 'active'
         ORDER BY COALESCE(folder_path, '') COLLATE NOCASE,
                  title COLLATE NOCASE,
                  url COLLATE NOCASE
        "#,
    )?;
    let bookmarks = rows
        .query_map([], |row| {
            Ok(ExportBookmark {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                note: row.get(4)?,
                tags: json_list(row.get::<_, String>(5)?),
                aliases: json_list(row.get::<_, String>(6)?),
                intents: json_list(row.get::<_, String>(7)?),
                folder_path: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                first_seen_at: row.get(11)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut counts = HashMap::<String, usize>::new();
    for bookmark in &bookmarks {
        *counts.entry(bookmark.folder_path.clone()).or_default() += 1;
    }
    let mut category_statement = conn.prepare(
        "SELECT path FROM categories WHERE status = 'active' ORDER BY path COLLATE NOCASE",
    )?;
    let mut categories = category_statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for folder in counts.keys() {
        if !categories.contains(folder) {
            categories.push(folder.clone());
        }
    }
    categories.sort_by(|left, right| {
        counts
            .get(right)
            .unwrap_or(&0)
            .cmp(counts.get(left).unwrap_or(&0))
            .then_with(|| left.to_lowercase().cmp(&right.to_lowercase()))
    });

    let now = Utc::now().timestamp().to_string();
    let mut lines = vec![
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>".to_owned(),
        "<!-- This is an automatically generated file.".to_owned(),
        "     It will be read and overwritten.".to_owned(),
        "     DO NOT EDIT! -->".to_owned(),
        r#"<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">"#.to_owned(),
        "<TITLE>Bookmarks</TITLE>".to_owned(),
        "<H1>Bookmarks</H1>".to_owned(),
        "<DL><p>".to_owned(),
    ];
    let mut indent = "    ".to_owned();
    if !top_folder.is_empty() {
        lines.push(format!(
            r#"{indent}<DT><H3 ADD_DATE="{now}" LAST_MODIFIED="{now}">{}</H3>"#,
            html_escape::encode_text(top_folder)
        ));
        lines.push(format!("{indent}<DL><p>"));
        indent = "        ".to_owned();
    }

    for category in &categories {
        let display = if category.is_empty() {
            "Unfiled"
        } else {
            category.as_str()
        };
        lines.push(format!(
            r#"{indent}<DT><H3 ADD_DATE="{now}" LAST_MODIFIED="{now}">{}</H3>"#,
            html_escape::encode_text(display)
        ));
        lines.push(format!("{indent}<DL><p>"));
        for bookmark in bookmarks
            .iter()
            .filter(|bookmark| &bookmark.folder_path == category)
        {
            let snapshot = export_snapshot(conn, &bookmark.id)?;
            let title = bookmark.title.as_deref().unwrap_or(&bookmark.url);
            let mut attrs = vec![
                format!(
                    r#"HREF="{}""#,
                    html_escape::encode_double_quoted_attribute(&bookmark.url)
                ),
                format!(
                    r#"ADD_DATE="{}""#,
                    html_epoch(
                        bookmark
                            .first_seen_at
                            .as_deref()
                            .unwrap_or(&bookmark.created_at)
                    )
                ),
                format!(r#"LAST_MODIFIED="{}""#, html_epoch(&bookmark.updated_at)),
            ];
            add_list_attr(&mut attrs, "TAGS", &bookmark.tags);
            add_list_attr(&mut attrs, "ALIASES", &bookmark.aliases);
            add_list_attr(&mut attrs, "INTENTS", &bookmark.intents);
            add_optional_attr(&mut attrs, "NOTE", bookmark.note.as_deref());
            add_optional_attr(&mut attrs, "DESCRIPTION", bookmark.description.as_deref());
            if let Some(snapshot) = &snapshot {
                add_optional_attr(&mut attrs, "SNAPSHOT_PATH", Some(&snapshot.artifact_path));
                add_optional_attr(
                    &mut attrs,
                    "SNAPSHOT_SHA256",
                    Some(&snapshot.content_sha256),
                );
                add_optional_attr(&mut attrs, "SNAPSHOT_KIND", Some(&snapshot.kind));
                add_optional_attr(
                    &mut attrs,
                    "SNAPSHOT_CAPTURED_AT",
                    snapshot.captured_at.as_deref(),
                );
                add_optional_attr(
                    &mut attrs,
                    "SNAPSHOT_BYTES",
                    snapshot
                        .byte_count
                        .as_ref()
                        .map(|value| value.to_string())
                        .as_deref(),
                );
                add_optional_attr(&mut attrs, "SNAPSHOT_TITLE", snapshot.title.as_deref());
            }
            lines.push(format!(
                r#"{indent}    <DT><A {}>{}</A>"#,
                attrs.join(" "),
                html_escape::encode_text(title)
            ));
            let mut description_parts = Vec::new();
            if let Some(note) = bookmark.note.as_deref().filter(|value| !value.is_empty()) {
                description_parts.push(note.to_owned());
            } else if let Some(description) = bookmark
                .description
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                description_parts.push(description.to_owned());
            }
            let mut metadata = Vec::new();
            if !bookmark.tags.is_empty() {
                metadata.push(format!("tags: {}", bookmark.tags.join(", ")));
            }
            if !bookmark.aliases.is_empty() {
                metadata.push(format!("aliases: {}", bookmark.aliases.join(", ")));
            }
            if !bookmark.intents.is_empty() {
                metadata.push(format!("intents: {}", bookmark.intents.join(", ")));
            }
            if !metadata.is_empty() {
                description_parts.push(metadata.join(" | "));
            }
            if !description_parts.is_empty() {
                lines.push(format!(
                    "{indent}    <DD>{}",
                    html_escape::encode_text(&description_parts.join(" / "))
                ));
            }
        }
        lines.push(format!("{indent}</DL><p>"));
    }
    if !top_folder.is_empty() {
        lines.push("    </DL><p>".to_owned());
    }
    lines.push("</DL><p>".to_owned());
    lines.push(String::new());
    Ok((lines.join("\n"), bookmarks.len(), categories.len()))
}

#[derive(Debug)]
struct ExportBookmark {
    id: String,
    url: String,
    title: Option<String>,
    description: Option<String>,
    note: Option<String>,
    tags: Vec<String>,
    aliases: Vec<String>,
    intents: Vec<String>,
    folder_path: String,
    created_at: String,
    updated_at: String,
    first_seen_at: Option<String>,
}

fn export_snapshot(conn: &Connection, bookmark_id: &str) -> Result<Option<SnapshotMetadata>> {
    conn.query_row(
        r#"
        SELECT artifact_path, content_sha256, kind, title, captured_at, byte_count
          FROM snapshots
         WHERE bookmark_id = ?1
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
    .optional()
    .map_err(Into::into)
}

fn common_root(items: &[ImportItem]) -> Option<String> {
    let folders = items
        .iter()
        .map(|item| normalize_category_path(Some(&item.folder_path)))
        .collect::<Vec<_>>();
    if folders.is_empty() || folders.iter().any(String::is_empty) {
        return None;
    }
    let roots = folders
        .iter()
        .filter_map(|folder| folder.split('/').next())
        .collect::<HashSet<_>>();
    (roots.len() == 1).then(|| roots.into_iter().next().unwrap_or_default().to_owned())
}

fn strip_common_root(items: &mut [ImportItem]) -> Option<String> {
    let root = common_root(items)?;
    for item in items {
        let folder = normalize_category_path(Some(&item.folder_path));
        item.folder_path = if folder == root {
            String::new()
        } else {
            folder
                .strip_prefix(&format!("{root}/"))
                .unwrap_or(&folder)
                .to_owned()
        };
    }
    Some(root)
}

fn sanitize_file_name(file_name: &str) -> String {
    let file_name = std::path::Path::new(file_name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bookmarks.html");
    let mut output = file_name
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || matches!(value, '.' | '_' | '-' | ' ') {
                value
            } else {
                '_'
            }
        })
        .take(180)
        .collect::<String>();
    if output.is_empty() {
        output = "bookmarks.html".to_owned();
    }
    output
}

fn snapshot_from_attrs(attrs: &HashMap<String, String>) -> Option<SnapshotMetadata> {
    let names = [
        "snapshot_path",
        "snapshot_sha256",
        "snapshot_kind",
        "snapshot_title",
        "snapshot_captured_at",
        "snapshot_bytes",
    ];
    if !names.iter().any(|name| attrs.contains_key(*name)) {
        return None;
    }
    Some(SnapshotMetadata {
        artifact_path: attrs.get("snapshot_path").cloned().unwrap_or_default(),
        content_sha256: attrs
            .get("snapshot_sha256")
            .map(|value| value.to_lowercase())
            .unwrap_or_default(),
        kind: attrs.get("snapshot_kind").cloned().unwrap_or_default(),
        title: attrs.get("snapshot_title").cloned(),
        captured_at: attrs.get("snapshot_captured_at").cloned(),
        byte_count: attrs
            .get("snapshot_bytes")
            .and_then(|value| value.parse::<i64>().ok()),
    })
}

fn parse_list(value: Option<&str>) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(value) = value {
        for item in value.replace(';', ",").split(',') {
            let item = item.trim();
            if !item.is_empty() && !result.iter().any(|existing| existing == item) {
                result.push(item.to_owned());
            }
        }
    }
    result
}

fn clean_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn json_list(value: String) -> Vec<String> {
    serde_json::from_str::<Vec<serde_json::Value>>(&value)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .filter(|item| !item.trim().is_empty())
        .collect()
}

fn add_list_attr(attrs: &mut Vec<String>, name: &str, values: &[String]) {
    if !values.is_empty() {
        add_optional_attr(attrs, name, Some(&values.join(",")));
    }
}

fn add_optional_attr(attrs: &mut Vec<String>, name: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        attrs.push(format!(
            r#"{name}="{}""#,
            html_escape::encode_double_quoted_attribute(value)
        ));
    }
}

fn html_epoch(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.timestamp().to_string())
        .unwrap_or_else(|_| Utc::now().timestamp().to_string())
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
