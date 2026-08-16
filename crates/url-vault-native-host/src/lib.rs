use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;
use url_vault_core::{SaveUrlInput, Vault, VaultError};

pub const HOST_NAME: &str = "com.suzukimakoto.codex_url_vault";
pub const EXTENSION_ID: &str = "fhijpanhohijhimeiajdgfhcpipbhkkh";
pub const BRIDGE_SOCKET_NAME: &str = ".browser-current-page.sock";
pub const MANIFEST_FILE_NAME: &str = "com.suzukimakoto.codex_url_vault.json";
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const BRIDGE_IO_TIMEOUT: Duration = Duration::from_secs(8);
const LISTENER_POLL_INTERVAL: Duration = Duration::from_millis(20);
const BRIDGE_DISCONNECTED_MESSAGE: &str = "browser extension disconnected before capture completed";
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Brave,
    Chrome,
}

impl Browser {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Brave => "brave",
            Self::Chrome => "chrome",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Brave => "Brave",
            Self::Chrome => "Google Chrome",
        }
    }

    pub fn native_messaging_directory(self, home: &Path) -> PathBuf {
        match self {
            Self::Brave => home.join(
                "Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts",
            ),
            Self::Chrome => {
                home.join("Library/Application Support/Google/Chrome/NativeMessagingHosts")
            }
        }
    }
}

impl FromStr for Browser {
    type Err = HostError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "brave" => Ok(Self::Brave),
            "chrome" => Ok(Self::Chrome),
            _ => Err(HostError::UnsupportedBrowser(value.to_owned())),
        }
    }
}

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
    #[error("unsupported browser: {0}; expected brave or chrome")]
    UnsupportedBrowser(String),
    #[error("browser capture bridge is already running")]
    BridgeAlreadyRunning,
    #[error("browser capture bridge is unavailable: {0}")]
    BridgeUnavailable(String),
    #[error("browser capture bridge protocol error: {0}")]
    BridgeProtocol(String),
    #[error("browser current-page capture failed ({code}): {message}")]
    Capture { code: String, message: String },
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
        source_browser: Browser,
    },
    CurrentPageResult {
        id: String,
        browser: Browser,
        ok: bool,
        #[serde(default)]
        url: String,
        #[serde(default)]
        title: String,
        tab_id: Option<i64>,
        window_id: Option<i64>,
        captured_at: Option<String>,
        error: Option<NativeError>,
    },
}

impl NativeRequest {
    fn id(&self) -> &str {
        match self {
            Self::Ping { id }
            | Self::ListCategories { id }
            | Self::SaveUrl { id, .. }
            | Self::CurrentPageResult { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeResponse {
    pub id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<NativeError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SaveCurrentBrowserPageInput {
    pub browser: Option<Browser>,
    pub folder_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BridgeRequest {
    SaveCurrentBrowserPage {
        id: String,
        browser: Option<Browser>,
        folder_path: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
        note: Option<String>,
    },
}

impl BridgeRequest {
    fn id(&self) -> &str {
        match self {
            Self::SaveCurrentBrowserPage { id, .. } => id,
        }
    }
}

struct PendingCapture {
    stream: UnixStream,
    input: SaveCurrentBrowserPageInput,
}

enum BridgeEvent {
    NativePayload(Vec<u8>),
    NativeClosed,
    NativeReadError(String),
    LocalPayload(Vec<u8>, UnixStream),
    ListenerError(String),
    ListenerStopped,
}

struct SocketGuard {
    path: PathBuf,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|metadata| metadata.file_type().is_socket()) {
            let _ = fs::remove_file(&self.path);
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

pub fn serve_bridge<R: Read + Send + 'static, W: Write>(
    reader: R,
    mut writer: W,
    vault: &Vault,
) -> Result<(), HostError> {
    let socket_path = bridge_socket_path(vault);
    let listener = bind_bridge_listener(&socket_path)?;
    let _socket_guard = SocketGuard {
        path: socket_path.clone(),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let (events_tx, events_rx) = mpsc::channel();

    let native_tx = events_tx.clone();
    thread::spawn(move || read_native_events(reader, native_tx));

    let listener_tx = events_tx.clone();
    let listener_stop = Arc::clone(&stop);
    thread::spawn(move || accept_local_requests(listener, listener_tx, listener_stop));
    drop(events_tx);

    run_bridge_events(events_rx, &mut writer, vault, stop.as_ref())
}

fn run_bridge_events(
    events_rx: mpsc::Receiver<BridgeEvent>,
    writer: &mut impl Write,
    vault: &Vault,
    stop: &AtomicBool,
) -> Result<(), HostError> {
    let mut pending = HashMap::<String, PendingCapture>::new();
    let mut terminal_result = None;
    loop {
        let event = match events_rx.recv() {
            Ok(event) => event,
            Err(_) => {
                if terminal_result.is_none() {
                    enter_bridge_terminal(
                        stop,
                        &mut pending,
                        &mut terminal_result,
                        Err(HostError::BridgeProtocol(
                            "bridge event channel closed".to_owned(),
                        )),
                    );
                }
                return terminal_result.expect("terminal result recorded");
            }
        };
        match event {
            BridgeEvent::NativePayload(payload) if terminal_result.is_none() => {
                let handled = match serde_json::from_slice::<NativeRequest>(&payload) {
                    Ok(NativeRequest::CurrentPageResult {
                        id,
                        browser,
                        ok,
                        url,
                        title,
                        tab_id,
                        window_id,
                        captured_at,
                        error,
                    }) => handle_current_page_result(
                        vault,
                        &mut pending,
                        CurrentPageCapture {
                            id,
                            browser,
                            ok,
                            url,
                            title,
                            tab_id,
                            window_id,
                            captured_at,
                            error,
                        },
                    ),
                    Ok(request) => {
                        let response = handle_request(vault, request);
                        write_frame(writer, &response)
                    }
                    Err(error) => Err(HostError::Json(error)),
                };
                if let Err(error) = handled {
                    enter_bridge_terminal(stop, &mut pending, &mut terminal_result, Err(error));
                }
            }
            BridgeEvent::NativePayload(_) => {}
            BridgeEvent::LocalPayload(payload, mut stream) => {
                let connected = terminal_result.is_none();
                let handled =
                    handle_local_request(&payload, &mut stream, writer, &mut pending, connected);
                if let Err(error) = handled {
                    if terminal_result.is_none() {
                        enter_bridge_terminal(stop, &mut pending, &mut terminal_result, Err(error));
                    }
                }
            }
            BridgeEvent::NativeClosed => {
                if terminal_result.is_none() {
                    enter_bridge_terminal(stop, &mut pending, &mut terminal_result, Ok(()));
                }
            }
            BridgeEvent::NativeReadError(error) => {
                if terminal_result.is_none() {
                    enter_bridge_terminal(
                        stop,
                        &mut pending,
                        &mut terminal_result,
                        Err(HostError::BridgeProtocol(error)),
                    );
                }
            }
            BridgeEvent::ListenerError(error) => {
                if terminal_result.is_none() {
                    enter_bridge_terminal(
                        stop,
                        &mut pending,
                        &mut terminal_result,
                        Err(HostError::BridgeUnavailable(error)),
                    );
                }
            }
            BridgeEvent::ListenerStopped => {
                if terminal_result.is_none() {
                    enter_bridge_terminal(
                        stop,
                        &mut pending,
                        &mut terminal_result,
                        Err(HostError::BridgeProtocol(
                            "bridge listener stopped before the native connection closed"
                                .to_owned(),
                        )),
                    );
                }
                return terminal_result.expect("terminal result recorded");
            }
        }
    }
}

fn enter_bridge_terminal(
    stop: &AtomicBool,
    pending: &mut HashMap<String, PendingCapture>,
    terminal_result: &mut Option<Result<(), HostError>>,
    result: Result<(), HostError>,
) {
    stop.store(true, Ordering::Release);
    for (id, mut capture) in pending.drain() {
        let disconnected =
            NativeResponse::failure(Some(id), "bridge_disconnected", BRIDGE_DISCONNECTED_MESSAGE);
        let _ = write_frame(&mut capture.stream, &disconnected);
    }
    *terminal_result = Some(result);
}

pub fn request_save_current_browser_page(
    vault: &Vault,
    input: SaveCurrentBrowserPageInput,
) -> Result<Value, HostError> {
    let socket_path = bridge_socket_path(vault);
    let mut stream = UnixStream::connect(&socket_path).map_err(|error| {
        HostError::BridgeUnavailable(format!("{}: {error}", socket_path.display()))
    })?;
    stream.set_read_timeout(Some(BRIDGE_IO_TIMEOUT))?;
    stream.set_write_timeout(Some(BRIDGE_IO_TIMEOUT))?;
    let id = format!(
        "mcp-{}-{}",
        std::process::id(),
        REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let request = BridgeRequest::SaveCurrentBrowserPage {
        id,
        browser: input.browser,
        folder_path: input.folder_path,
        tags: input.tags,
        note: input.note,
    };
    write_frame(&mut stream, &request)?;
    let payload = read_frame(&mut stream)?.ok_or_else(|| {
        HostError::BridgeProtocol("bridge closed without a capture response".to_owned())
    })?;
    let response = serde_json::from_slice::<NativeResponse>(&payload)?;
    if response.ok {
        return response
            .result
            .ok_or_else(|| HostError::BridgeProtocol("capture result is missing".to_owned()));
    }
    let error = response.error.unwrap_or(NativeError {
        code: "capture_failed".to_owned(),
        message: "browser current-page capture failed".to_owned(),
    });
    Err(HostError::Capture {
        code: error.code,
        message: error.message,
    })
}

pub fn bridge_socket_path(vault: &Vault) -> PathBuf {
    vault.home().join(BRIDGE_SOCKET_NAME)
}

pub fn handle_request(vault: &Vault, request: NativeRequest) -> NativeResponse {
    let id = request.id().to_owned();
    let result = match request {
        NativeRequest::Ping { .. } => Ok(json!({
            "host": HOST_NAME,
            "version": env!("CARGO_PKG_VERSION"),
            "bridge_socket": bridge_socket_path(vault),
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
            source_browser,
            ..
        } => save_url(vault, &url, &title, &category, tags, &note, source_browser),
        NativeRequest::CurrentPageResult { .. } => Err(HostError::BridgeProtocol(
            "capture results require the persistent bridge".to_owned(),
        )),
    };
    match result {
        Ok(value) => NativeResponse::success(id, value),
        Err(error) => NativeResponse::failure(Some(id), error_code(&error), error.to_string()),
    }
}

struct CurrentPageCapture {
    id: String,
    browser: Browser,
    ok: bool,
    url: String,
    title: String,
    tab_id: Option<i64>,
    window_id: Option<i64>,
    captured_at: Option<String>,
    error: Option<NativeError>,
}

fn handle_current_page_result(
    vault: &Vault,
    pending: &mut HashMap<String, PendingCapture>,
    capture: CurrentPageCapture,
) -> Result<(), HostError> {
    let Some(mut request) = pending.remove(&capture.id) else {
        return Err(HostError::BridgeProtocol(format!(
            "unknown current-page request id: {}",
            capture.id
        )));
    };
    if request
        .input
        .browser
        .is_some_and(|browser| browser != capture.browser)
    {
        let expected = request.input.browser.expect("checked browser");
        let response = NativeResponse::failure(
            Some(capture.id),
            "browser_mismatch",
            format!(
                "requested {} but the connected extension is {}",
                expected.display_name(),
                capture.browser.display_name()
            ),
        );
        write_frame(&mut request.stream, &response)?;
        return Ok(());
    }
    let response = if capture.ok {
        let tab_id = capture.tab_id.ok_or_else(|| {
            HostError::BridgeProtocol("capture result is missing tab_id".to_owned())
        })?;
        let window_id = capture.window_id.ok_or_else(|| {
            HostError::BridgeProtocol("capture result is missing window_id".to_owned())
        })?;
        let mut result = save_url(
            vault,
            &capture.url,
            &capture.title,
            request.input.folder_path.as_deref().unwrap_or_default(),
            request.input.tags,
            request.input.note.as_deref().unwrap_or_default(),
            capture.browser,
        )?;
        let result_object = result.as_object_mut().ok_or_else(|| {
            HostError::BridgeProtocol("save result is not a JSON object".to_owned())
        })?;
        result_object.insert(
            "capture".to_owned(),
            json!({
                "browser": capture.browser,
                "tab_id": tab_id,
                "window_id": window_id,
                "captured_at": capture.captured_at,
            }),
        );
        NativeResponse::success(capture.id, result)
    } else {
        let error = capture.error.unwrap_or(NativeError {
            code: "capture_failed".to_owned(),
            message: "browser current-page capture failed".to_owned(),
        });
        NativeResponse::failure(Some(capture.id), &error.code, error.message)
    };
    if let Err(error) = write_frame(&mut request.stream, &response) {
        eprintln!("url-vault-native-host: local capture response failed: {error}");
    }
    Ok(())
}

fn handle_local_request(
    payload: &[u8],
    stream: &mut UnixStream,
    writer: &mut impl Write,
    pending: &mut HashMap<String, PendingCapture>,
    connected: bool,
) -> Result<(), HostError> {
    let request = match serde_json::from_slice::<BridgeRequest>(payload) {
        Ok(request) => request,
        Err(error) => {
            let response =
                NativeResponse::failure(None, "invalid_bridge_request", error.to_string());
            write_frame(stream, &response)?;
            return Ok(());
        }
    };
    if !connected {
        let response = NativeResponse::failure(
            Some(request.id().to_owned()),
            "bridge_disconnected",
            BRIDGE_DISCONNECTED_MESSAGE,
        );
        write_frame(stream, &response)?;
        return Ok(());
    }
    match request {
        BridgeRequest::SaveCurrentBrowserPage {
            id,
            browser,
            folder_path,
            tags,
            note,
        } => {
            if pending.contains_key(&id) {
                let response = NativeResponse::failure(
                    Some(id),
                    "duplicate_request",
                    "capture request id is already pending",
                );
                write_frame(stream, &response)?;
                return Ok(());
            }
            let capture_command = json!({
                "type": "capture_current_page",
                "id": id,
                "browser": browser,
            });
            pending.insert(
                id.clone(),
                PendingCapture {
                    stream: stream.try_clone()?,
                    input: SaveCurrentBrowserPageInput {
                        browser,
                        folder_path,
                        tags: normalize_list(tags),
                        note,
                    },
                },
            );
            if let Err(error) = write_frame(writer, &capture_command) {
                pending.remove(&id);
                let response = NativeResponse::failure(
                    Some(id),
                    "bridge_disconnected",
                    BRIDGE_DISCONNECTED_MESSAGE,
                );
                let _ = write_frame(stream, &response);
                return Err(error);
            }
        }
    }
    Ok(())
}

fn read_native_events(mut reader: impl Read, events: mpsc::Sender<BridgeEvent>) {
    loop {
        match read_frame(&mut reader) {
            Ok(Some(payload)) => {
                if events.send(BridgeEvent::NativePayload(payload)).is_err() {
                    break;
                }
            }
            Ok(None) => {
                let _ = events.send(BridgeEvent::NativeClosed);
                break;
            }
            Err(error) => {
                let _ = events.send(BridgeEvent::NativeReadError(error.to_string()));
                break;
            }
        }
    }
}

fn accept_local_requests(
    listener: UnixListener,
    events: mpsc::Sender<BridgeEvent>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(BRIDGE_IO_TIMEOUT));
                let _ = stream.set_write_timeout(Some(BRIDGE_IO_TIMEOUT));
                match read_frame(&mut stream) {
                    Ok(Some(payload)) => {
                        if events
                            .send(BridgeEvent::LocalPayload(payload, stream))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let response = NativeResponse::failure(
                            None,
                            "invalid_bridge_request",
                            error.to_string(),
                        );
                        let _ = write_frame(&mut stream, &response);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(LISTENER_POLL_INTERVAL);
            }
            Err(error) => {
                let _ = events.send(BridgeEvent::ListenerError(error.to_string()));
                break;
            }
        }
    }
    let _ = events.send(BridgeEvent::ListenerStopped);
}

fn bind_bridge_listener(path: &Path) -> Result<UnixListener, HostError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(HostError::BridgeAlreadyRunning);
        }
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_socket() {
            return Err(HostError::BridgeUnavailable(format!(
                "{} exists and is not a Unix socket",
                path.display()
            )));
        }
        fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

fn save_url(
    vault: &Vault,
    url: &str,
    title: &str,
    category: &str,
    tags: Vec<String>,
    note: &str,
    source_browser: Browser,
) -> Result<Value, HostError> {
    let parsed = Url::parse(url).map_err(|_| HostError::UnsupportedUrl)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(HostError::UnsupportedUrl);
    }
    let result = vault.save_url_with_operation(SaveUrlInput {
        url: parsed.to_string(),
        title: optional_text(title),
        note: optional_text(note),
        tags: normalize_list(tags),
        source_type: Some("browser_extension".to_owned()),
        source_browser: Some(source_browser.as_str().to_owned()),
        folder_path: optional_text(category),
        status: Some("active".to_owned()),
        ..SaveUrlInput::default()
    })?;
    Ok(json!({
        "operation": result.operation,
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
        HostError::UnsupportedBrowser(_) => "unsupported_browser",
        HostError::Vault(VaultError::NotFound(_)) => "not_found",
        HostError::Vault(VaultError::Ambiguous(_)) => "ambiguous",
        HostError::Vault(VaultError::InvalidInput(_)) => "invalid_input",
        HostError::Vault(_) => "vault_error",
        HostError::MessageTooLarge => "message_too_large",
        HostError::Json(_) => "invalid_json",
        HostError::Io(_) => "io_error",
        HostError::BridgeAlreadyRunning => "bridge_already_running",
        HostError::BridgeUnavailable(_) => "bridge_unavailable",
        HostError::BridgeProtocol(_) => "bridge_protocol",
        HostError::Capture { .. } => "capture_failed",
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

fn write_frame(writer: &mut impl Write, value: &impl Serialize) -> Result<(), HostError> {
    let payload = serde_json::to_vec(value)?;
    let length = u32::try_from(payload.len()).map_err(|_| HostError::MessageTooLarge)?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

pub fn browser_manifest(browser: Browser, host_path: &Path) -> Result<Value, HostError> {
    if !host_path.is_absolute() {
        return Err(HostError::BridgeUnavailable(
            "native host path must be absolute".to_owned(),
        ));
    }
    Ok(json!({
        "name": HOST_NAME,
        "description": format!("Codex URL Vault {} capture host", browser.display_name()),
        "path": host_path.to_string_lossy(),
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{EXTENSION_ID}/")],
    }))
}

pub fn install_browser_manifest(
    browser: Browser,
    host_path: &Path,
    home: &Path,
) -> Result<PathBuf, HostError> {
    let manifest = browser_manifest(browser, host_path)?;
    let directory = browser.native_messaging_directory(home);
    fs::create_dir_all(&directory)?;
    let destination = directory.join(MANIFEST_FILE_NAME);
    let temporary = directory.join(format!(".{MANIFEST_FILE_NAME}.tmp-{}", std::process::id()));
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    fs::write(&temporary, bytes)?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o644))?;
    fs::rename(&temporary, &destination)?;
    for other_browser in [Browser::Brave, Browser::Chrome] {
        if other_browser == browser {
            continue;
        }
        let other_manifest = other_browser
            .native_messaging_directory(home)
            .join(MANIFEST_FILE_NAME);
        if other_manifest.exists() {
            fs::remove_file(other_manifest)?;
        }
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn frame_round_trip_preserves_response() {
        let response = NativeResponse::success("one", json!({ "pong": true }));
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &response).expect("write native frame");
        let payload = read_frame(&mut io::Cursor::new(bytes))
            .expect("read native frame")
            .expect("frame");
        let value: Value = serde_json::from_slice(&payload).expect("response JSON");
        assert_eq!(value["id"], "one");
        assert_eq!(value["result"]["pong"], true);
    }

    #[test]
    fn save_request_uses_the_shared_core_and_reports_operation() {
        let home = tempfile::tempdir().expect("temporary Vault");
        let vault = Vault::at(home.path()).expect("Vault");
        vault.init().expect("initialize Vault");
        let request = || NativeRequest::SaveUrl {
            id: "save-one".to_owned(),
            url: "https://example.test/extension".to_owned(),
            title: "Extension capture".to_owned(),
            category: "Browser/Inbox".to_owned(),
            tags: vec!["brave".to_owned(), "capture".to_owned()],
            note: "Saved by test".to_owned(),
            source_browser: Browser::Brave,
        };
        let created = handle_request(&vault, request());
        let updated = handle_request(&vault, request());
        assert!(created.ok);
        assert_eq!(created.result.as_ref().unwrap()["operation"], "created");
        assert_eq!(updated.result.as_ref().unwrap()["operation"], "updated");
        let saved = vault
            .search_urls("Extension capture", 5)
            .expect("search saved URL");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].folder_path.as_deref(), Some("Browser/Inbox"));
        assert_eq!(saved[0].source_type, "browser_extension");
        assert_eq!(saved[0].source_browser.as_deref(), Some("brave"));
    }

    #[test]
    fn persistent_bridge_captures_and_saves_the_current_page() {
        let home = tempfile::tempdir().expect("temporary Vault");
        let vault = Vault::at(home.path()).expect("Vault");
        vault.init().expect("initialize Vault");
        let client_vault = vault.clone();
        let host_vault = vault.clone();
        let (host_stream, mut extension_stream) =
            UnixStream::pair().expect("native messaging pair");
        let host_reader = host_stream.try_clone().expect("host reader");
        let host = thread::spawn(move || serve_bridge(host_reader, host_stream, &host_vault));
        let extension = thread::spawn(move || {
            for _ in 0..2 {
                let payload = read_frame(&mut extension_stream)
                    .expect("read capture command")
                    .expect("capture command");
                let command: Value =
                    serde_json::from_slice(&payload).expect("capture command JSON");
                assert_eq!(command["type"], "capture_current_page");
                write_frame(
                    &mut extension_stream,
                    &json!({
                        "type": "current_page_result",
                        "id": command["id"],
                        "browser": "chrome",
                        "ok": true,
                        "url": "https://example.test/current",
                        "title": "Current Chrome page",
                        "tab_id": 17,
                        "window_id": 4,
                        "captured_at": "2026-07-31T00:00:00.000Z"
                    }),
                )
                .expect("write capture result");
            }
        });
        wait_for_socket(&bridge_socket_path(&client_vault));

        let input = SaveCurrentBrowserPageInput {
            browser: Some(Browser::Chrome),
            folder_path: Some("Browser/Voice".to_owned()),
            tags: vec!["voice".to_owned()],
            note: None,
        };
        let created = request_save_current_browser_page(&client_vault, input.clone())
            .expect("create current page");
        let updated =
            request_save_current_browser_page(&client_vault, input).expect("update current page");
        assert_eq!(created["operation"], "created");
        assert_eq!(updated["operation"], "updated");
        assert_eq!(created["bookmark"]["title"], "Current Chrome page");
        assert_eq!(created["bookmark"]["folder_path"], "Browser/Voice");
        assert_eq!(created["capture"]["tab_id"], 17);
        assert_eq!(created["capture"]["browser"], "chrome");

        extension.join().expect("extension thread");
        host.join()
            .expect("host thread")
            .expect("persistent bridge");
        assert!(!bridge_socket_path(&client_vault).exists());
    }

    #[test]
    fn native_disconnect_resolves_accepted_local_request_in_both_event_orders() {
        for native_closes_first in [true, false] {
            let home = tempfile::tempdir().expect("temporary Vault");
            let vault = Vault::at(home.path()).expect("Vault");
            vault.init().expect("initialize Vault");
            let (host_stream, mut client_stream) = UnixStream::pair().expect("local bridge pair");
            client_stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .expect("client read timeout");
            let payload = serde_json::to_vec(&BridgeRequest::SaveCurrentBrowserPage {
                id: "capture-disconnect".to_owned(),
                browser: Some(Browser::Brave),
                folder_path: None,
                tags: Vec::new(),
                note: None,
            })
            .expect("bridge request JSON");
            let local_event = BridgeEvent::LocalPayload(payload, host_stream);
            let (events_tx, events_rx) = mpsc::channel();
            if native_closes_first {
                events_tx
                    .send(BridgeEvent::NativeClosed)
                    .expect("queue native close");
                events_tx.send(local_event).expect("queue local request");
            } else {
                events_tx.send(local_event).expect("queue local request");
                events_tx
                    .send(BridgeEvent::NativeClosed)
                    .expect("queue native close");
            }
            events_tx
                .send(BridgeEvent::ListenerStopped)
                .expect("queue listener stop");
            drop(events_tx);

            let stop = AtomicBool::new(false);
            let mut native_output = Vec::new();
            run_bridge_events(events_rx, &mut native_output, &vault, &stop)
                .expect("clean native disconnect");

            let response_payload = read_frame(&mut client_stream)
                .expect("read disconnect response")
                .expect("framed disconnect response");
            let response: NativeResponse =
                serde_json::from_slice(&response_payload).expect("disconnect response JSON");
            assert_eq!(response.id.as_deref(), Some("capture-disconnect"));
            assert!(!response.ok);
            let error = response.error.expect("structured disconnect error");
            assert_eq!(error.code, "bridge_disconnected");
            assert_eq!(error.message, BRIDGE_DISCONNECTED_MESSAGE);
            assert!(
                read_frame(&mut client_stream)
                    .expect("read bridge EOF after response")
                    .is_none(),
                "accepted request received more than one response"
            );
            assert!(stop.load(Ordering::Acquire));
        }
    }

    #[test]
    fn browser_selection_rejects_a_different_connected_extension() {
        let home = tempfile::tempdir().expect("temporary Vault");
        let vault = Vault::at(home.path()).expect("Vault");
        vault.init().expect("initialize Vault");
        let (host_stream, mut client_stream) = UnixStream::pair().expect("bridge pair");
        let mut pending = HashMap::from([(
            "capture-one".to_owned(),
            PendingCapture {
                stream: host_stream,
                input: SaveCurrentBrowserPageInput {
                    browser: Some(Browser::Brave),
                    ..SaveCurrentBrowserPageInput::default()
                },
            },
        )]);

        handle_current_page_result(
            &vault,
            &mut pending,
            CurrentPageCapture {
                id: "capture-one".to_owned(),
                browser: Browser::Chrome,
                ok: true,
                url: "https://example.test/current".to_owned(),
                title: "Current page".to_owned(),
                tab_id: Some(17),
                window_id: Some(4),
                captured_at: None,
                error: None,
            },
        )
        .expect("handle browser mismatch");

        let payload = read_frame(&mut client_stream)
            .expect("read mismatch response")
            .expect("mismatch response");
        let response: NativeResponse =
            serde_json::from_slice(&payload).expect("mismatch response JSON");
        assert!(!response.ok);
        assert_eq!(
            response.error.as_ref().map(|error| error.code.as_str()),
            Some("browser_mismatch")
        );
        assert!(pending.is_empty());
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
                url: "brave://settings/".to_owned(),
                title: String::new(),
                category: String::new(),
                tags: Vec::new(),
                note: String::new(),
                source_browser: Browser::Brave,
            },
        );
        assert!(!response.ok);
        assert_eq!(
            response.error.as_ref().map(|error| error.code.as_str()),
            Some("unsupported_url")
        );
    }

    #[test]
    fn generated_manifests_are_bound_to_each_browser_and_the_extension() {
        let host_path = Path::new("/Applications/Codex URL Vault.app/host");
        for browser in [Browser::Brave, Browser::Chrome] {
            let manifest = browser_manifest(browser, host_path).expect("browser manifest");
            assert_eq!(manifest["name"], HOST_NAME);
            assert_eq!(manifest["path"], host_path.to_string_lossy().as_ref());
            assert!(
                manifest["description"]
                    .as_str()
                    .expect("description")
                    .contains(browser.display_name())
            );
            assert_eq!(
                manifest["allowed_origins"][0],
                format!("chrome-extension://{EXTENSION_ID}/")
            );
        }
    }

    #[test]
    fn installs_only_the_selected_browser_manifest() {
        let home = tempfile::tempdir().expect("temporary home");
        let host_path = Path::new("/Applications/Codex URL Vault.app/host");
        let brave_manifest_path = Browser::Brave
            .native_messaging_directory(home.path())
            .join(MANIFEST_FILE_NAME);
        fs::create_dir_all(brave_manifest_path.parent().expect("Brave manifest parent"))
            .expect("create previous Brave registration directory");
        fs::write(&brave_manifest_path, b"previous registration")
            .expect("write previous Brave registration");
        let installed = install_browser_manifest(Browser::Chrome, host_path, home.path())
            .expect("install Chrome manifest");
        assert_eq!(
            installed,
            Browser::Chrome
                .native_messaging_directory(home.path())
                .join(MANIFEST_FILE_NAME)
        );
        assert!(installed.exists());
        assert!(!brave_manifest_path.exists());
    }

    fn wait_for_socket(path: &Path) {
        let started = Instant::now();
        while !path.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "bridge socket was not created"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
}
