use std::fs;

use tempfile::TempDir;
use url_vault_core::{
    ApplyImportInput, SaveSnapshotInput, SaveUrlInput, Vault, bookmark_id, canonicalize_url,
};

fn vault() -> (TempDir, Vault) {
    let home = tempfile::tempdir().expect("temporary Vault");
    let vault = Vault::at(home.path()).expect("Vault path");
    vault.init().expect("initialize Vault");
    (home, vault)
}

fn save(vault: &Vault, url: &str, title: &str, category: &str) -> String {
    vault
        .save_url(SaveUrlInput {
            url: url.to_owned(),
            title: Some(title.to_owned()),
            folder_path: Some(category.to_owned()),
            tags: vec!["reference".to_owned()],
            aliases: vec!["durable alias".to_owned()],
            intents: vec!["find this later".to_owned()],
            ..SaveUrlInput::default()
        })
        .expect("save URL")
        .item
        .id
}

#[test]
fn canonicalization_and_identity_match_the_frozen_contract() {
    let canonical = canonicalize_url(" Example.COM/path/?Q=Mixed#section ");
    assert_eq!(canonical, "https://example.com/path?Q=Mixed");
    assert_eq!(
        canonicalize_url("https://Example.COM?Q=Mixed"),
        "https://example.com/?Q=Mixed"
    );
    assert_eq!(bookmark_id(&canonical).len(), 24);
    assert_eq!(bookmark_id(&canonical), bookmark_id(&canonical));
}

#[test]
fn mutations_keep_sqlite_and_canonical_html_in_sync() {
    let (home, vault) = vault();
    let id = save(
        &vault,
        "https://example.com/reference/",
        "Reference",
        "Research/Primary",
    );

    let listed = vault
        .list_urls(Some("Research/Primary"), None)
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);

    let html = fs::read_to_string(vault.canonical_html_path()).expect("canonical HTML");
    assert!(html.contains(r#"HREF="https://example.com/reference/""#));
    assert!(html.contains(r#"ALIASES="durable alias""#));
    assert!(html.contains(r#"INTENTS="find this later""#));
    assert!(home.path().join("vault.sqlite").is_file());
}

#[test]
fn bookmark_html_preview_apply_and_round_trip_preserve_categories() {
    let (_home, vault) = vault();
    let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<DL><p>
  <DT><H3>Imported Root</H3>
  <DL><p>
    <DT><H3>Docs</H3>
    <DL><p>
      <DT><A HREF="https://docs.example.test/" ADD_DATE="1">Example Docs</A>
    </DL><p>
  </DL><p>
</DL><p>
"#;
    let preview = vault
        .preview_bookmark_import(html, "../Bookmarks.html", true)
        .expect("preview");
    assert_eq!(preview.unique_count, 1);
    assert_eq!(preview.stripped_root.as_deref(), Some("Imported Root"));
    assert_eq!(preview.file_name, "Bookmarks.html");

    let applied = vault
        .apply_bookmark_import(ApplyImportInput {
            html: html.to_owned(),
            file_name: "../Bookmarks.html".to_owned(),
            source_browser: "brave".to_owned(),
            source_profile: "Default".to_owned(),
            preferred_browser: Some("brave".to_owned()),
            mode: "merge".to_owned(),
            strip_common_root: true,
            expected_sha256: preview.content_sha256,
            confirm_reset: false,
        })
        .expect("apply");
    assert_eq!(applied.created, 1);
    assert_eq!(
        vault.list_urls(Some("Docs"), None).expect("category").len(),
        1
    );

    vault.refresh_db().expect("rebuild from canonical HTML");
    assert_eq!(
        vault
            .list_urls(Some("Docs"), None)
            .expect("round-trip")
            .len(),
        1
    );
}

#[test]
fn verified_snapshot_text_is_searchable_but_not_exposed_by_list_or_search() {
    let (home, vault) = vault();
    let id = save(
        &vault,
        "https://source.example.test/disappearing",
        "Volatile source",
        "",
    );
    let content = "# Durable fact\n\nmercury-saffron survives locally.\n";
    let saved = vault
        .save_snapshot(SaveSnapshotInput {
            target: id.clone(),
            content: content.to_owned(),
            kind: "semantic".to_owned(),
            title: Some("Saved meaning".to_owned()),
            captured_at: Some("2026-07-30T00:00:00Z".to_owned()),
        })
        .expect("save snapshot");
    assert!(home.path().join(&saved.snapshot.artifact_path).is_file());

    let search = vault.search_urls("mercury-saffron", 5).expect("search");
    assert_eq!(search[0].id, id);
    assert_eq!(
        search[0]
            .snapshot
            .as_ref()
            .and_then(|value| value.content.as_ref()),
        None
    );
    assert_eq!(
        vault.list_urls(None, None).expect("list")[0]
            .snapshot
            .as_ref()
            .and_then(|value| value.content.as_ref()),
        None
    );

    let payload = vault.get_snapshot(&id).expect("explicit snapshot read");
    assert_eq!(
        payload.snapshot.and_then(|value| value.content).as_deref(),
        Some(content)
    );
}

#[test]
fn archiving_a_category_moves_bookmarks_to_unfiled() {
    let (_home, vault) = vault();
    save(
        &vault,
        "https://example.test/category",
        "Categorized",
        "Temporary",
    );
    vault
        .archive_category("Temporary", "")
        .expect("archive category");
    let bookmarks = vault.list_urls(Some(""), None).expect("Unfiled");
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0].folder_path.as_deref(), Some(""));
}

#[test]
fn deleting_a_url_removes_its_record_and_canonical_html_entry() {
    let (home, vault) = vault();
    let url = "https://example.test/delete-me";
    let id = save(&vault, url, "Delete me", "Temporary");

    let deleted = vault.delete_url(&id).expect("delete URL");

    assert_eq!(deleted.item.id, id);
    assert!(vault.list_urls(None, None).expect("list after delete").is_empty());
    let connection = rusqlite::Connection::open(home.path().join("vault.sqlite"))
        .expect("open Vault database");
    let record_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM bookmarks WHERE id = ?1",
            [&id],
            |row| row.get(0),
        )
        .expect("count deleted record");
    assert_eq!(record_count, 0);
    let html = fs::read_to_string(vault.canonical_html_path()).expect("canonical HTML");
    assert!(!html.contains(url));
}

#[test]
fn opening_by_exact_bookmark_id_does_not_fall_back_to_fuzzy_search() {
    let (_home, vault) = vault();
    let id = save(
        &vault,
        "https://example.test/open-by-id",
        "Open by identifier",
        "Reference",
    );

    let result = vault
        .open_url(&id, None, true, "test", None)
        .expect("open exact bookmark id");

    assert_eq!(result.url, "https://example.test/open-by-id");
    assert_eq!(result.bookmark.map(|bookmark| bookmark.id), Some(id));
}
