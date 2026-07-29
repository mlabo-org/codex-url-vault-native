use std::ffi::{CStr, CString, c_char};
use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use url_vault_core::{ApplyImportInput, SaveSnapshotInput, SaveUrlInput, UpdateUrlInput, Vault};

type FfiResult<T> = Result<T, String>;

fn vault(home: *const c_char) -> FfiResult<Vault> {
    let configured = optional_string(home)?;
    match configured {
        Some(path) => Vault::at(PathBuf::from(path)).map_err(|error| error.to_string()),
        None => Vault::from_env().map_err(|error| error.to_string()),
    }
}

fn required_string(value: *const c_char, name: &str) -> FfiResult<String> {
    optional_string(value)?.ok_or_else(|| format!("{name} is required"))
}

fn optional_string(value: *const c_char) -> FfiResult<Option<String>> {
    if value.is_null() {
        return Ok(None);
    }
    // SAFETY: The Swift caller passes a live NUL-terminated UTF-8 buffer for the duration
    // of each synchronous call.
    let value = unsafe { CStr::from_ptr(value) }
        .to_str()
        .map_err(|error| format!("invalid UTF-8 input: {error}"))?;
    Ok(Some(value.to_owned()))
}

fn decode<T: DeserializeOwned>(value: *const c_char, name: &str) -> FfiResult<T> {
    let value = required_string(value, name)?;
    serde_json::from_str(&value).map_err(|error| format!("invalid {name}: {error}"))
}

fn response<T: Serialize>(
    operation: impl FnOnce() -> FfiResult<T> + std::panic::UnwindSafe,
) -> *mut c_char {
    let value = match std::panic::catch_unwind(operation) {
        Ok(Ok(data)) => json!({"ok": true, "data": data}),
        Ok(Err(error)) => json!({"ok": false, "error": error}),
        Err(_) => json!({"ok": false, "error": "native core panicked"}),
    };
    let encoded = serde_json::to_string(&value)
        .unwrap_or_else(|_| r#"{"ok":false,"error":"response encoding failed"}"#.to_owned())
        .replace('\0', "\u{fffd}");
    CString::new(encoded)
        .expect("NUL characters were replaced")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_free_string(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: `value` must be a pointer returned by one of this library's exported
        // operations and must be released exactly once.
        unsafe {
            drop(CString::from_raw(value));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_init(home: *const c_char) -> *mut c_char {
    response(|| vault(home)?.init().map_err(|error| error.to_string()))
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_list_urls(
    home: *const c_char,
    category: *const c_char,
    limit: u32,
) -> *mut c_char {
    response(|| {
        let category = optional_string(category)?;
        vault(home)?
            .list_urls(category.as_deref(), (limit > 0).then_some(limit as usize))
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_search_urls(
    home: *const c_char,
    query: *const c_char,
    limit: u32,
) -> *mut c_char {
    response(|| {
        vault(home)?
            .search_urls(
                &required_string(query, "query")?,
                usize::try_from(limit.max(1)).unwrap_or(10),
            )
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_get_url(home: *const c_char, target: *const c_char) -> *mut c_char {
    response(|| {
        vault(home)?
            .get_url(&required_string(target, "target")?)
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_list_categories(home: *const c_char) -> *mut c_char {
    response(|| {
        vault(home)?
            .list_categories()
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_save_url(
    home: *const c_char,
    input_json: *const c_char,
) -> *mut c_char {
    response(|| {
        let input: SaveUrlInput = decode(input_json, "save URL input")?;
        vault(home)?
            .save_url(input)
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_update_url(
    home: *const c_char,
    target: *const c_char,
    changes_json: *const c_char,
) -> *mut c_char {
    response(|| {
        let changes: UpdateUrlInput = decode(changes_json, "URL changes")?;
        vault(home)?
            .update_url(&required_string(target, "target")?, changes)
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_move_url(
    home: *const c_char,
    target: *const c_char,
    category: *const c_char,
) -> *mut c_char {
    response(|| {
        vault(home)?
            .move_url(
                &required_string(target, "target")?,
                &required_string(category, "category")?,
            )
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_archive_url(home: *const c_char, target: *const c_char) -> *mut c_char {
    response(|| {
        vault(home)?
            .archive_url(&required_string(target, "target")?)
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_create_category(
    home: *const c_char,
    path: *const c_char,
    label: *const c_char,
    note: *const c_char,
) -> *mut c_char {
    response(|| {
        let label = optional_string(label)?;
        let note = optional_string(note)?;
        vault(home)?
            .create_category(
                &required_string(path, "path")?,
                label.as_deref(),
                note.as_deref(),
            )
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_rename_category(
    home: *const c_char,
    old_path: *const c_char,
    new_path: *const c_char,
) -> *mut c_char {
    response(|| {
        vault(home)?
            .rename_category(
                &required_string(old_path, "old path")?,
                &required_string(new_path, "new path")?,
            )
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_archive_category(
    home: *const c_char,
    path: *const c_char,
    move_to: *const c_char,
) -> *mut c_char {
    response(|| {
        vault(home)?
            .archive_category(
                &required_string(path, "path")?,
                &required_string(move_to, "move target")?,
            )
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_preview_import(
    home: *const c_char,
    html: *const c_char,
    file_name: *const c_char,
    strip_common_root: bool,
) -> *mut c_char {
    response(|| {
        vault(home)?
            .preview_bookmark_import(
                &required_string(html, "bookmark HTML")?,
                &required_string(file_name, "file name")?,
                strip_common_root,
            )
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_apply_import(
    home: *const c_char,
    input_json: *const c_char,
) -> *mut c_char {
    response(|| {
        let input: ApplyImportInput = decode(input_json, "bookmark import input")?;
        vault(home)?
            .apply_bookmark_import(input)
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_save_snapshot(
    home: *const c_char,
    input_json: *const c_char,
) -> *mut c_char {
    response(|| {
        let input: SaveSnapshotInput = decode(input_json, "snapshot input")?;
        vault(home)?
            .save_snapshot(input)
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_get_snapshot(
    home: *const c_char,
    target: *const c_char,
) -> *mut c_char {
    response(|| {
        vault(home)?
            .get_snapshot(&required_string(target, "target")?)
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_open_url(
    home: *const c_char,
    target: *const c_char,
    browser: *const c_char,
    dry_run: bool,
) -> *mut c_char {
    response(|| {
        let browser = optional_string(browser)?;
        vault(home)?
            .open_url(
                &required_string(target, "target")?,
                browser.as_deref(),
                dry_run,
                "native_app",
                None,
            )
            .map_err(|error| error.to_string())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn url_vault_verify_snapshots(
    home: *const c_char,
    target: *const c_char,
) -> *mut c_char {
    response(|| {
        let target = optional_string(target)?;
        vault(home)?
            .verify_snapshots(target.as_deref())
            .map_err(|error| error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_returns_an_owned_json_envelope() {
        let home = tempfile::tempdir().expect("tempdir");
        let home = CString::new(home.path().to_string_lossy().as_bytes()).expect("path");
        let output = url_vault_init(home.as_ptr());
        assert!(!output.is_null());
        // SAFETY: The operation returned a live C string.
        let value = unsafe { CStr::from_ptr(output) }.to_str().expect("UTF-8");
        assert!(value.contains(r#""ok":true"#));
        url_vault_free_string(output);
    }
}
