use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub canonical_url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub intents: Vec<String>,
    pub source_type: String,
    pub source_browser: Option<String>,
    pub source_profile: Option<String>,
    pub folder_path: Option<String>,
    pub preferred_browser: Option<String>,
    pub project: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub last_opened_at: Option<String>,
    pub open_count: i64,
    pub seen_in_latest_import: bool,
    pub missing_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Snapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Category {
    pub path: String,
    pub label: String,
    pub note: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub display: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub bookmark_id: String,
    pub artifact_path: String,
    pub sha256: String,
    pub kind: String,
    pub title: Option<String>,
    pub captured_at: Option<String>,
    pub byte_count: Option<i64>,
    pub status: String,
    pub verified_at: Option<String>,
    pub updated_at: String,
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SaveUrlInput {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub note: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub intents: Vec<String>,
    pub source_type: Option<String>,
    pub source_browser: Option<String>,
    pub source_profile: Option<String>,
    pub folder_path: Option<String>,
    pub preferred_browser: Option<String>,
    pub project: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUrlInput {
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub note: Option<String>,
    pub tags: Option<Vec<String>>,
    pub aliases: Option<Vec<String>>,
    pub intents: Option<Vec<String>>,
    pub folder_path: Option<String>,
    pub preferred_browser: Option<String>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationResult<T> {
    pub item: T,
    pub canonical_html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestResult {
    pub query: String,
    pub candidates: Vec<Bookmark>,
    pub open_directly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub file_name: String,
    pub content_sha256: String,
    pub parsed_count: usize,
    pub unique_count: usize,
    pub duplicate_count: usize,
    pub folder_count: usize,
    pub folders: Vec<String>,
    pub common_root: Option<String>,
    pub stripped_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyImportInput {
    pub html: String,
    pub file_name: String,
    pub source_browser: String,
    #[serde(default)]
    pub source_profile: String,
    pub preferred_browser: Option<String>,
    #[serde(default = "default_import_mode")]
    pub mode: String,
    #[serde(default)]
    pub strip_common_root: bool,
    pub expected_sha256: String,
    #[serde(default)]
    pub confirm_reset: bool,
}

fn default_import_mode() -> String {
    "merge".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub import_id: String,
    pub db: String,
    pub mode: String,
    pub source_browser: String,
    pub source_profile: String,
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub canonical_html: String,
    pub file_name: String,
    pub content_sha256: String,
    pub parsed_count: usize,
    pub unique_count: usize,
    pub duplicate_count: usize,
    pub folder_count: usize,
    pub folders: Vec<String>,
    pub common_root: Option<String>,
    pub stripped_root: Option<String>,
    pub backup: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSnapshotInput {
    pub target: String,
    pub content: String,
    #[serde(default = "default_snapshot_kind")]
    pub kind: String,
    pub title: Option<String>,
    pub captured_at: Option<String>,
}

fn default_snapshot_kind() -> String {
    "semantic".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotResult {
    pub bookmark: BookmarkIdentity,
    pub snapshot: Snapshot,
    pub canonical_html: String,
    pub replaced_snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkIdentity {
    pub id: String,
    pub title: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPayload {
    pub bookmark: BookmarkIdentity,
    pub snapshot: Option<Snapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotVerification {
    pub checked: usize,
    pub valid: usize,
    pub invalid: usize,
    pub results: Vec<SnapshotPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenResult {
    pub url: String,
    pub browser: String,
    pub bookmark: Option<Bookmark>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitResult {
    pub db: String,
    pub bookmarks: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportItem {
    pub url: String,
    pub title: String,
    pub folder_path: String,
    pub add_date: Option<String>,
    pub last_modified: Option<String>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub intents: Vec<String>,
    pub note: Option<String>,
    pub description: Option<String>,
    pub snapshot: Option<SnapshotMetadata>,
}

#[derive(Debug, Clone)]
pub(crate) struct SnapshotMetadata {
    pub artifact_path: String,
    pub content_sha256: String,
    pub kind: String,
    pub title: Option<String>,
    pub captured_at: Option<String>,
    pub byte_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedImport {
    pub preview: ImportPreview,
    pub items: Vec<ImportItem>,
}
