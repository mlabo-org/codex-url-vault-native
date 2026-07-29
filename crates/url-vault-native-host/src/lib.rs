use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;
use url_vault_core::{SaveUrlInput, Vault, VaultError};

pub const HOST_NAME: &str = "com.suzukimakoto.codex_url_vault";
pub const EXTENSION_ID: &str = "fhijpanhohijhimeiajdgfhcpipbhkkh";
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("native message exceeds the 1 MiB limit")]
    MessageTooLarge,
    #[error("native host only accepts http and https URLs")]
    UnsupportedUrl,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NativeRequest {
    Ping {
        id: String,
    },
    ListCategories {
        id: String,
    },
    SaveUrl {
        id: String,
        url: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        note: String,
    },
}

impl NativeRequest {
    fn id(&self) -> &str {
        match self {
            Self::Ping { id } | Self::ListCategories { id } | Self::SaveUrl { id, .. } => id,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NativeResponse {
    pub id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<NativeError>,
}

#[derive(Debug, Serialize)]
pub struct NativeError {
    pub code: String,
    pub message: String,
}

impl NativeResponse {
    fn success(id: impl Into<String>, result: Value) -> Self {
        Self {
            id: Some(id.into()),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    fn failure(id: Option<String>, code: &str, message: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(NativeError {
                code: code.to_owned(),
                message: message.into(),
            }),
        }
    }
}

pub fn serve<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    vault: &Vault,
) -> Result<(), HostError> {
    while let Some(payload) = read_frame(&mut reader)? {
        let response = match serde_json::from_slice::<NativeRequest>(&payload) {
            Ok(request) => handle_request(vault, request),
            Err(error) => NativeResponse::failure(None, "invalid_request", error.to_string()),
        };
        write_frame(&mut writer, &response)?;
    }
    Ok(())
}

pub fn handle_request(vault: &Vault, request: NativeRequest) -> NativeResponse {
    let id = request.id().to_owned();
    let result = match request {
        NativeRequest::Ping { .. } => Ok(json!({
            "host": HOST_NAME,
            "version": env!("CARGO_PKG_VERSION"),
        })),
        NativeRequest::ListCategories { .. } => vault
            .list_categories()
            .map(|categories| json!({ "categories": categories }))
            .map_err(HostError::from),
        NativeRequest::SaveUrl {
            url,
            title,
            category,
            tags,
            note,
            ..
        } => save_url(vault, &url, &title, &category, tags, &note),
    };
    match result {
        Ok(value) => NativeResponse::success(id, value),
        Err(error) => NativeResponse::failure(Some(id), error_code(&error), error.to_string()),
    }
}

fn save_url(
    vault: &Vault,
    url: &str,
    title: &str,
    category: &str,
    tags: Vec<String>,
    note: &str,
) -> Result<Value, HostError> {
    let parsed = Url::parse(url).map_err(|_| HostError::UnsupportedUrl)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(HostError::UnsupportedUrl);
    }
    let result = vault.save_url(SaveUrlInput {
        url: parsed.to_string(),
        title: optional_text(title),
        note: optional_text(note),
        tags: normalize_list(tags),
        source_type: Some("browser_extension".to_owned()),
        source_browser: Some("chrome".to_owned()),
        folder_path: optional_text(category),
        status: Some("active".to_owned()),
        ..SaveUrlInput::default()
    })?;
    Ok(json!({
        "bookmark": {
            "id": result.item.id,
            "url": result.item.url,
            "title": result.item.title,
            "folder_path": result.item.folder_path,
            "tags": result.item.tags,
        }
    }))
}

fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn normalize_list(values: Vec<String>) -> Vec<String> {
    values.into_iter().fold(Vec::new(), |mut output, value| {
        let value = value.trim().to_owned();
        if !value.is_empty() && !output.contains(&value) {
            output.push(value);
        }
        output
    })
}

fn error_code(error: &HostError) -> &'static str {
    match error {
        HostError::UnsupportedUrl => "unsupported_url",
        HostError::Vault(VaultError::NotFound(_)) => "not_found",
        HostError::Vault(VaultError::Ambiguous(_)) => "ambiguous",
        HostError::Vault(VaultError::InvalidInput(_)) => "invalid_input",
        HostError::Vault(_) => "vault_error",
        HostError::MessageTooLarge => "message_too_large",
        HostError::Json(_) => "invalid_json",
        HostError::Io(_) => "io_error",
    }
}

fn read_frame(reader: &mut impl Read) -> Result<Option<Vec<u8>>, HostError> {
    let mut length = [0_u8; 4];
    match reader.read_exact(&mut length) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_MESSAGE_BYTES {
        return Err(HostError::MessageTooLarge);
    }
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

fn write_frame(writer: &mut impl Write, response: &NativeResponse) -> Result<(), HostError> {
    let payload = serde_json::to_vec(response)?;
    let length = u32::try_from(payload.len()).map_err(|_| HostError::MessageTooLarge)?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

pub fn chrome_manifest(host_path: &str) -> Value {
    json!({
        "name": HOST_NAME,
        "description": "Codex URL Vault Chrome capture host",
        "path": host_path,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{EXTENSION_ID}/")],
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn frame_round_trip_preserves_response() {
        let response = NativeResponse::success("one", json!({ "pong": true }));
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &response).expect("write native frame");
        let payload = read_frame(&mut Cursor::new(bytes))
            .expect("read native frame")
            .expect("frame");
        let value: Value = serde_json::from_slice(&payload).expect("response JSON");
        assert_eq!(value["id"], "one");
        assert_eq!(value["result"]["pong"], true);
    }

    #[test]
    fn save_request_uses_the_shared_core() {
        let home = tempfile::tempdir().expect("temporary Vault");
        let vault = Vault::at(home.path()).expect("Vault");
        vault.init().expect("initialize Vault");
        let response = handle_request(
            &vault,
            NativeRequest::SaveUrl {
                id: "save-one".to_owned(),
                url: "https://example.test/extension".to_owned(),
                title: "Extension capture".to_owned(),
                category: "Browser/Inbox".to_owned(),
                tags: vec!["chrome".to_owned(), "capture".to_owned()],
                note: "Saved by test".to_owned(),
            },
        );
        assert!(response.ok);
        let saved = vault
            .search_urls("Extension capture", 5)
            .expect("search saved URL");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].folder_path.as_deref(), Some("Browser/Inbox"));
        assert_eq!(saved[0].source_type, "browser_extension");
    }

    #[test]
    fn rejects_non_web_urls() {
        let home = tempfile::tempdir().expect("temporary Vault");
        let vault = Vault::at(home.path()).expect("Vault");
        vault.init().expect("initialize Vault");
        let response = handle_request(
            &vault,
            NativeRequest::SaveUrl {
                id: "save-two".to_owned(),
                url: "chrome://settings/".to_owned(),
                title: String::new(),
                category: String::new(),
                tags: Vec::new(),
                note: String::new(),
            },
        );
        assert!(!response.ok);
        assert_eq!(
            response.error.as_ref().map(|error| error.code.as_str()),
            Some("unsupported_url")
        );
    }

    #[test]
    fn generated_manifest_is_bound_to_the_extension() {
        let manifest = chrome_manifest("/Applications/Codex URL Vault.app/host");
        assert_eq!(manifest["name"], HOST_NAME);
        assert_eq!(
            manifest["allowed_origins"][0],
            format!("chrome-extension://{EXTENSION_ID}/")
        );
    }
}
