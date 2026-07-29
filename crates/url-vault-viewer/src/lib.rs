use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer};
use serde::Serialize;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use url::Url;
use url_vault_core::{ApplyImportInput, SaveUrlInput, UpdateUrlInput, Vault, VaultError};

const INDEX_HTML: &[u8] = include_bytes!("../assets/index.html");
const STYLES_CSS: &[u8] = include_bytes!("../assets/styles.css");
const APP_JS: &[u8] = include_bytes!("../assets/app.js");
const GO_HTML: &[u8] = include_bytes!("../assets/go.html");
const GO_JS: &[u8] = include_bytes!("../assets/go.js");
const OPEN_HTML: &[u8] = include_bytes!("../assets/open.html");
const OPEN_JS: &[u8] = include_bytes!("../assets/open.js");
const MAX_JSON_BYTES: u64 = 24 * 1024 * 1024;
const MAX_READER_BYTES: u64 = 3 * 1024 * 1024;

type HttpResponse = Response<Cursor<Vec<u8>>>;

static VIEWER_SESSION: Mutex<Option<ViewerInfo>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerInfo {
    pub viewer_url: String,
    pub origin: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ViewerError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("viewer server error: {0}")]
    Server(String),
    #[error("invalid viewer request: {0}")]
    InvalidRequest(String),
    #[error("reader fetch failed: {0}")]
    Fetch(String),
}

#[derive(Clone)]
struct ViewerContext {
    vault: Vault,
    token: String,
    origin: String,
}

pub fn start_iab_viewer() -> Result<ViewerInfo, ViewerError> {
    let mut session = VIEWER_SESSION
        .lock()
        .map_err(|_| ViewerError::Server("viewer session lock is poisoned".to_owned()))?;
    if let Some(info) = session.as_ref() {
        return Ok(info.clone());
    }

    let vault = Vault::from_env()?;
    vault.init()?;
    let server =
        Server::http(("127.0.0.1", 0)).map_err(|error| ViewerError::Server(error.to_string()))?;
    let address = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| ViewerError::Server("viewer did not bind an IP socket".to_owned()))?;
    let origin = format!("http://127.0.0.1:{}", address.port());
    let token = random_token()?;
    let info = ViewerInfo {
        viewer_url: format!("{origin}/?session={token}"),
        origin: origin.clone(),
    };
    let context = ViewerContext {
        vault,
        token,
        origin,
    };

    thread::Builder::new()
        .name("url-vault-iab-viewer".to_owned())
        .spawn(move || serve(server, context))
        .map_err(ViewerError::Io)?;

    *session = Some(info.clone());
    Ok(info)
}

fn serve(server: Server, context: ViewerContext) {
    for request in server.incoming_requests() {
        if let Err(error) = handle_request(request, &context) {
            eprintln!("url-vault-viewer: {error}");
        }
    }
}

fn handle_request(mut request: Request, context: &ViewerContext) -> Result<(), ViewerError> {
    let request_url = Url::parse(&format!("{}{}", context.origin, request.url()))
        .map_err(|error| ViewerError::InvalidRequest(error.to_string()))?;
    if !valid_host(&request, &context.origin) {
        return respond(request, json_error(403, "invalid_host"));
    }

    let query_token = query_value(&request_url, "session");
    let has_cookie = authorized_cookie(&request, &context.token);
    let authorized = has_cookie || query_token.as_deref() == Some(context.token.as_str());
    if !authorized {
        return respond(request, json_error(403, "viewer_session_required"));
    }

    if request.method() != &Method::Get
        && let Some(origin) = header_value(&request, "Origin")
        && origin != context.origin
    {
        return respond(request, json_error(403, "invalid_origin"));
    }

    let path = request_url.path().to_owned();
    let method = request.method().clone();
    let mut response = if path.starts_with("/api/") {
        route_api(&mut request, context, &request_url, &method, &path)
            .unwrap_or_else(error_response)
    } else {
        route_page(context, &request_url, &method, &path).unwrap_or_else(error_response)
    };

    if !has_cookie && query_token.as_deref() == Some(context.token.as_str()) {
        response.add_header(header(
            "Set-Cookie",
            &format!(
                "vault_session={}; HttpOnly; SameSite=Strict; Path=/",
                context.token
            ),
        ));
    }
    respond(request, response)
}

fn route_page(
    context: &ViewerContext,
    request_url: &Url,
    method: &Method,
    path: &str,
) -> Result<HttpResponse, ViewerError> {
    if method != &Method::Get {
        return Ok(json_error(405, "method_not_allowed"));
    }
    match path {
        "/" | "/index.html" => Ok(static_response(INDEX_HTML, "text/html; charset=utf-8")),
        "/styles.css" => Ok(static_response(STYLES_CSS, "text/css; charset=utf-8")),
        "/app.js" => Ok(static_response(APP_JS, "text/javascript; charset=utf-8")),
        "/go.html" => Ok(static_response(GO_HTML, "text/html; charset=utf-8")),
        "/go.js" => Ok(static_response(GO_JS, "text/javascript; charset=utf-8")),
        "/open.html" => Ok(static_response(OPEN_HTML, "text/html; charset=utf-8")),
        "/open.js" => Ok(static_response(OPEN_JS, "text/javascript; charset=utf-8")),
        "/open-external" => open_external(context, request_url),
        "/reader" => reader_response(request_url),
        _ => Ok(json_error(404, "not_found")),
    }
}

fn route_api(
    request: &mut Request,
    context: &ViewerContext,
    request_url: &Url,
    method: &Method,
    path: &str,
) -> Result<HttpResponse, ViewerError> {
    match (method, path) {
        (&Method::Get, "/api/bookmarks") => {
            let category = query_value(request_url, "category");
            Ok(json_response(
                200,
                json!({
                    "bookmarks": context.vault.list_urls(category.as_deref(), None)?,
                    "categories": context.vault.list_categories()?,
                }),
            ))
        }
        (&Method::Get, "/api/categories") => Ok(json_response(
            200,
            json!({ "categories": context.vault.list_categories()? }),
        )),
        (&Method::Post, "/api/bookmarks") => {
            let input = save_input_from_viewer(&read_json(request)?)?;
            let result = context.vault.save_url(input)?;
            Ok(json_response(
                200,
                json!({ "ok": true, "bookmark": result.item }),
            ))
        }
        (&Method::Post, "/api/categories") => {
            let payload = read_json(request)?;
            let result = context.vault.create_category(
                required_value(&payload, "path")?,
                optional_value(&payload, "label"),
                optional_value(&payload, "note"),
            )?;
            Ok(json_response(
                200,
                json!({ "ok": true, "category": result.item }),
            ))
        }
        (&Method::Patch, "/api/categories") => {
            let payload = read_json(request)?;
            let old_path = required_value(&payload, "old_path")?;
            let new_path = required_value(&payload, "new_path")?;
            let result = context.vault.rename_category(old_path, new_path)?;
            Ok(json_response(
                200,
                json!({
                    "ok": true,
                    "old_path": old_path,
                    "new_path": result.item.path,
                }),
            ))
        }
        (&Method::Delete, "/api/categories") => {
            let path = query_value(request_url, "path")
                .ok_or_else(|| ViewerError::InvalidRequest("missing_path".to_owned()))?;
            let move_to = query_value(request_url, "move_to").unwrap_or_default();
            let result = context.vault.archive_category(&path, &move_to)?;
            Ok(json_response(
                200,
                json!({
                    "ok": true,
                    "path": result.item.path,
                    "move_to": move_to,
                }),
            ))
        }
        (&Method::Post, "/api/import/preview") => {
            let payload = read_json(request)?;
            let preview = context.vault.preview_bookmark_import(
                required_value(&payload, "content")?,
                required_value(&payload, "file_name")?,
                bool_value(&payload, "strip_common_root"),
            )?;
            Ok(json_response(
                200,
                json!({ "ok": true, "preview": preview }),
            ))
        }
        (&Method::Post, "/api/import") => {
            let payload = read_json(request)?;
            let input = ApplyImportInput {
                html: required_value(&payload, "content")?.to_owned(),
                file_name: required_value(&payload, "file_name")?.to_owned(),
                source_browser: required_value(&payload, "source_browser")?.to_owned(),
                source_profile: optional_value(&payload, "source_profile")
                    .unwrap_or_default()
                    .to_owned(),
                preferred_browser: optional_value(&payload, "preferred_browser").map(str::to_owned),
                mode: optional_value(&payload, "mode")
                    .unwrap_or("merge")
                    .to_owned(),
                strip_common_root: bool_value(&payload, "strip_common_root"),
                expected_sha256: required_value(&payload, "expected_sha256")?.to_owned(),
                confirm_reset: bool_value(&payload, "confirm_reset"),
            };
            let result = context.vault.apply_bookmark_import(input)?;
            Ok(json_response(200, json!({ "ok": true, "result": result })))
        }
        _ => route_bookmark_api(request, context, method, path),
    }
}

fn route_bookmark_api(
    request: &mut Request,
    context: &ViewerContext,
    method: &Method,
    path: &str,
) -> Result<HttpResponse, ViewerError> {
    let Some(remainder) = path.strip_prefix("/api/bookmarks/") else {
        return Ok(json_error(404, "not_found"));
    };

    if method == &Method::Get
        && let Some(target) = remainder.strip_suffix("/snapshot")
    {
        let payload = context.vault.get_snapshot(target)?;
        if payload.snapshot.is_none() {
            return Ok(json_error(404, "snapshot_not_found"));
        }
        return Ok(json_response(
            200,
            json!({
                "ok": true,
                "bookmark": payload.bookmark,
                "snapshot": payload.snapshot,
            }),
        ));
    }

    if method == &Method::Post
        && let Some(target) = remainder.strip_suffix("/move")
    {
        let payload = read_json(request)?;
        let result = context
            .vault
            .move_url(target, required_value(&payload, "category")?)?;
        return Ok(json_response(
            200,
            json!({ "ok": true, "bookmark": result.item }),
        ));
    }

    match method {
        &Method::Patch => {
            let changes = update_input_from_viewer(&read_json(request)?);
            let result = context.vault.update_url(remainder, changes)?;
            Ok(json_response(
                200,
                json!({ "ok": true, "bookmark": result.item }),
            ))
        }
        &Method::Delete => {
            context.vault.archive_url(remainder)?;
            Ok(json_response(
                200,
                json!({ "ok": true, "deleted_id": remainder }),
            ))
        }
        _ => Ok(json_error(404, "not_found")),
    }
}

fn open_external(context: &ViewerContext, request_url: &Url) -> Result<HttpResponse, ViewerError> {
    let target = query_value(request_url, "url")
        .ok_or_else(|| ViewerError::InvalidRequest("missing_url".to_owned()))?;
    let result = context.vault.open_url(
        &target,
        Some("default"),
        false,
        "iab_viewer",
        Some("external"),
    )?;
    Ok(json_response(
        200,
        json!({
            "ok": true,
            "browser": result.browser,
            "url": result.url,
        }),
    ))
}

fn reader_response(request_url: &Url) -> Result<HttpResponse, ViewerError> {
    let target = query_value(request_url, "url")
        .ok_or_else(|| ViewerError::InvalidRequest("missing_url".to_owned()))?;
    let parsed_target =
        Url::parse(&target).map_err(|error| ViewerError::InvalidRequest(error.to_string()))?;
    if !matches!(parsed_target.scheme(), "http" | "https") {
        return Ok(html_response(
            400,
            &reader_page("Preview failed", &target, "<p>Unsupported URL.</p>"),
        ));
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(12))
        .build();
    let response = agent
        .get(&target)
        .set(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/124.0 Safari/537.36",
        )
        .call()
        .map_err(|error| ViewerError::Fetch(error.to_string()))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_READER_BYTES)
        .read_to_end(&mut bytes)?;
    let source = String::from_utf8_lossy(&bytes);
    let extracted = extract_reader(&source, &parsed_target);
    let requested_title = query_value(request_url, "title").unwrap_or_default();
    let title = if extracted.title.is_empty() {
        if requested_title.is_empty() {
            target.as_str()
        } else {
            requested_title.as_str()
        }
    } else {
        extracted.title.as_str()
    };
    let body = if extracted.body.is_empty() {
        "<p>No readable text was extracted. Use Open IAB or your default browser for the original page.</p>"
    } else {
        extracted.body.as_str()
    };
    Ok(html_response(200, &reader_page(title, &target, body)))
}

fn reader_page(title: &str, target: &str, body: &str) -> String {
    let title_text = html_escape::encode_text(title);
    let target_text = html_escape::encode_text(target);
    let target_attr = html_escape::encode_double_quoted_attribute(target);
    format!(
        r#"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <base href="{target_attr}">
  <title>{title_text}</title>
  <style>
    body {{ margin:0; background:#f6f7f7; color:#162024; font:17px/1.75 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }}
    main {{ max-width:820px; margin:0 auto; padding:34px 22px 70px; }}
    h1,h2,h3 {{ line-height:1.25; }} h1 {{ font-size:30px; margin:0 0 10px; }}
    h2 {{ margin-top:34px; font-size:24px; }} h3 {{ margin-top:26px; font-size:20px; }}
    p,li,blockquote {{ overflow-wrap:anywhere; }} a {{ color:#0b7d68; }}
    img {{ display:block; max-width:100%; height:auto; margin:18px 0; border-radius:8px; }}
    blockquote {{ margin:20px 0; padding-left:16px; border-left:4px solid #56c6a9; color:#46545b; }}
    pre {{ overflow:auto; padding:14px; background:#11181d; color:#edf2f5; border-radius:8px; }}
    .source {{ margin-bottom:26px; color:#66747c; font-size:13px; overflow-wrap:anywhere; }}
  </style>
</head>
<body>
  <main>
    <h1>{title_text}</h1>
    <p class="source">{target_text}</p>
    <article>{body}</article>
  </main>
</body>
</html>"#
    )
}

fn read_json(request: &mut Request) -> Result<Value, ViewerError> {
    let mut body = Vec::new();
    request
        .as_reader()
        .take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut body)?;
    if body.len() as u64 > MAX_JSON_BYTES {
        return Err(ViewerError::InvalidRequest("request_too_large".to_owned()));
    }
    if body.is_empty() {
        return Ok(json!({}));
    }
    Ok(serde_json::from_slice(&body)?)
}

fn required_value<'a>(value: &'a Value, key: &str) -> Result<&'a str, ViewerError> {
    optional_value(value, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ViewerError::InvalidRequest(format!("missing_{key}")))
}

fn optional_value<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn bool_value(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn save_input_from_viewer(value: &Value) -> Result<SaveUrlInput, ViewerError> {
    Ok(SaveUrlInput {
        url: required_value(value, "url")?.to_owned(),
        title: optional_owned(value, "title"),
        description: optional_owned(value, "description"),
        note: optional_owned(value, "note"),
        tags: list_value(value, "tags"),
        aliases: list_value(value, "aliases"),
        intents: list_value(value, "intents"),
        source_type: optional_owned(value, "source_type"),
        source_browser: optional_owned(value, "source_browser"),
        source_profile: optional_owned(value, "source_profile"),
        folder_path: optional_owned(value, "folder_path")
            .or_else(|| optional_owned(value, "category")),
        preferred_browser: optional_owned(value, "preferred_browser"),
        project: optional_owned(value, "project"),
        status: optional_owned(value, "status"),
    })
}

fn update_input_from_viewer(value: &Value) -> UpdateUrlInput {
    UpdateUrlInput {
        url: owned_if_present(value, "url"),
        title: owned_if_present(value, "title"),
        description: owned_if_present(value, "description"),
        note: owned_if_present(value, "note"),
        tags: value.get("tags").map(|_| list_value(value, "tags")),
        aliases: value.get("aliases").map(|_| list_value(value, "aliases")),
        intents: value.get("intents").map(|_| list_value(value, "intents")),
        folder_path: owned_if_present(value, "folder_path")
            .or_else(|| owned_if_present(value, "category")),
        preferred_browser: owned_if_present(value, "preferred_browser"),
        project: owned_if_present(value, "project"),
    }
}

fn optional_owned(value: &Value, key: &str) -> Option<String> {
    optional_value(value, key)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
}

fn owned_if_present(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .map(str::to_owned)
}

fn list_value(value: &Value, key: &str) -> Vec<String> {
    let raw = value.get(key);
    let candidates = match raw {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(items)) => items
            .replace(';', ",")
            .split(',')
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    candidates
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .fold(Vec::new(), |mut items, item| {
            if !items.contains(&item) {
                items.push(item);
            }
            items
        })
}

fn query_value(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

fn valid_host(request: &Request, origin: &str) -> bool {
    let expected = origin.trim_start_matches("http://");
    header_value(request, "Host").is_some_and(|host| {
        host.eq_ignore_ascii_case(expected)
            || host.eq_ignore_ascii_case(&expected.replacen("127.0.0.1", "localhost", 1))
    })
}

fn authorized_cookie(request: &Request, token: &str) -> bool {
    header_value(request, "Cookie").is_some_and(|cookies| {
        cookies
            .split(';')
            .map(str::trim)
            .any(|cookie| cookie == format!("vault_session={token}"))
    })
}

fn header_value<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

fn random_token() -> Result<String, ViewerError> {
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn respond(request: Request, response: HttpResponse) -> Result<(), ViewerError> {
    request.respond(response).map_err(ViewerError::Io)
}

fn static_response(body: &[u8], content_type: &str) -> HttpResponse {
    response(200, body.to_vec(), content_type)
}

fn html_response(status: u16, body: &str) -> HttpResponse {
    response(status, body.as_bytes().to_vec(), "text/html; charset=utf-8")
}

fn json_response(status: u16, value: Value) -> HttpResponse {
    response(
        status,
        serde_json::to_vec(&value).unwrap_or_else(|_| b"{\"ok\":false}".to_vec()),
        "application/json; charset=utf-8",
    )
}

fn json_error(status: u16, message: &str) -> HttpResponse {
    json_response(status, json!({ "ok": false, "error": message }))
}

fn error_response(error: ViewerError) -> HttpResponse {
    match error {
        ViewerError::Vault(VaultError::NotFound(message)) => json_error(404, &message),
        ViewerError::Vault(VaultError::Ambiguous(message)) => json_error(409, &message),
        ViewerError::Vault(VaultError::InvalidInput(message))
        | ViewerError::InvalidRequest(message) => json_error(400, &message),
        ViewerError::Fetch(message) => html_response(
            502,
            &reader_page(
                "Preview failed",
                "",
                &format!("<p>{}</p>", html_escape::encode_text(&message)),
            ),
        ),
        other => json_error(500, &other.to_string()),
    }
}

fn response(status: u16, body: Vec<u8>, content_type: &str) -> HttpResponse {
    Response::from_data(body)
        .with_status_code(StatusCode(status))
        .with_header(header("Content-Type", content_type))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("Pragma", "no-cache"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header("Referrer-Policy", "no-referrer"))
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid HTTP header")
}

#[derive(Default)]
struct ReaderState {
    in_title: bool,
    in_body: bool,
    skip_depth: usize,
    title: String,
    body: String,
    link_stack: Vec<bool>,
}

#[derive(Default)]
struct ReaderSink {
    state: RefCell<ReaderState>,
    base: RefCell<Option<Url>>,
}

impl TokenSink for ReaderSink {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<Self::Handle> {
        let mut state = self.state.borrow_mut();
        match token {
            Token::TagToken(tag) => {
                let name = tag.name.as_ref();
                if tag.kind == TagKind::StartTag && name == "title" {
                    state.in_title = true;
                    return TokenSinkResult::Continue;
                }
                if tag.kind == TagKind::EndTag && name == "title" {
                    state.in_title = false;
                    return TokenSinkResult::Continue;
                }
                if tag.kind == TagKind::StartTag && name == "body" {
                    state.in_body = true;
                    return TokenSinkResult::Continue;
                }
                if tag.kind == TagKind::EndTag && name == "body" {
                    state.in_body = false;
                    return TokenSinkResult::Continue;
                }

                if matches!(
                    name,
                    "script"
                        | "style"
                        | "noscript"
                        | "svg"
                        | "canvas"
                        | "iframe"
                        | "object"
                        | "embed"
                ) {
                    if tag.kind == TagKind::StartTag {
                        state.skip_depth += 1;
                    } else if state.skip_depth > 0 {
                        state.skip_depth -= 1;
                    }
                    return TokenSinkResult::Continue;
                }
                if !state.in_body || state.skip_depth > 0 {
                    return TokenSinkResult::Continue;
                }

                let attrs = tag
                    .attrs
                    .iter()
                    .map(|attribute| {
                        (
                            attribute.name.local.to_string(),
                            attribute.value.to_string(),
                        )
                    })
                    .collect::<HashMap<_, _>>();
                let opening = tag.kind == TagKind::StartTag;
                match name {
                    "h1" | "h2" | "h3" | "p" | "li" | "blockquote" | "pre" | "strong" | "em"
                    | "b" | "i" | "code" | "ul" | "ol" => {
                        if opening {
                            state.body.push_str(&format!("<{name}>"));
                        } else {
                            state.body.push_str(&format!("</{name}>"));
                        }
                    }
                    "div" | "main" | "section" | "article" => {
                        state.body.push_str(if opening { "<p>" } else { "</p>" });
                    }
                    "br" if opening => state.body.push_str("<br>"),
                    "hr" if opening => state.body.push_str("<hr>"),
                    "a" if opening => {
                        let safe = attrs.get("href").and_then(|href| self.resolve_http(href));
                        if let Some(url) = safe {
                            state.body.push_str(&format!(
                                r#"<a href="{}" target="_top" rel="noreferrer">"#,
                                html_escape::encode_double_quoted_attribute(url.as_str())
                            ));
                            state.link_stack.push(true);
                        } else {
                            state.body.push_str("<span>");
                            state.link_stack.push(false);
                        }
                    }
                    "a" => {
                        let closing_tag = if state.link_stack.pop().unwrap_or(false) {
                            "</a>"
                        } else {
                            "</span>"
                        };
                        state.body.push_str(closing_tag);
                    }
                    "img" if opening => {
                        if let Some(url) = attrs
                            .get("src")
                            .or_else(|| attrs.get("data-src"))
                            .and_then(|src| self.resolve_http(src))
                        {
                            let alt = attrs.get("alt").map(String::as_str).unwrap_or_default();
                            state.body.push_str(&format!(
                                r#"<img src="{}" alt="{}" loading="lazy">"#,
                                html_escape::encode_double_quoted_attribute(url.as_str()),
                                html_escape::encode_double_quoted_attribute(alt)
                            ));
                        }
                    }
                    _ => {}
                }
            }
            Token::CharacterTokens(value) => {
                if state.in_title {
                    if !state.title.is_empty() {
                        state.title.push(' ');
                    }
                    state.title.push_str(value.trim());
                } else if state.in_body && state.skip_depth == 0 {
                    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !text.is_empty() {
                        state.body.push_str(&html_escape::encode_text(&text));
                        state.body.push(' ');
                    }
                }
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

impl ReaderSink {
    fn resolve_http(&self, value: &str) -> Option<Url> {
        let resolved = self.base.borrow().as_ref()?.join(value).ok()?;
        matches!(resolved.scheme(), "http" | "https").then_some(resolved)
    }
}

struct ReaderExtract {
    title: String,
    body: String,
}

fn extract_reader(source: &str, base: &Url) -> ReaderExtract {
    let sink = ReaderSink {
        base: RefCell::new(Some(base.clone())),
        ..ReaderSink::default()
    };
    let input = BufferQueue::default();
    input.push_back(StrTendril::from(source));
    let tokenizer = Tokenizer::new(sink, Default::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let state = tokenizer.sink.state.into_inner();
    ReaderExtract {
        title: state.title.split_whitespace().collect::<Vec<_>>().join(" "),
        body: state.body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_extraction_keeps_safe_content_and_drops_scripts() {
        let base = Url::parse("https://example.test/articles/one").expect("base URL");
        let result = extract_reader(
            "<html><head><title>Example Reader</title></head><body><h1>Heading</h1><script>alert(1)</script><p>Useful <a href=\"/more\">text</a>.</p></body></html>",
            &base,
        );
        assert_eq!(result.title, "Example Reader");
        assert!(result.body.contains("<h1>Heading"));
        assert!(result.body.contains("https://example.test/more"));
        assert!(!result.body.contains("alert"));
    }

    #[test]
    fn generated_session_is_long_and_hex_encoded() {
        let token = random_token().expect("session token");
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|character| character.is_ascii_hexdigit()));
    }

    #[test]
    fn socket_address_is_loopback() {
        let address: std::net::SocketAddr = "127.0.0.1:49152".parse().expect("socket");
        assert!(address.ip().is_loopback());
    }

    #[test]
    fn compatible_viewer_payload_maps_category_and_text_lists() {
        let payload = json!({
            "url": "https://example.test/",
            "title": "Example",
            "category": "Research/Web",
            "tags": "rust, viewer; local",
            "aliases": "sample, example",
            "intents": ""
        });
        let save = save_input_from_viewer(&payload).expect("viewer save input");
        assert_eq!(save.folder_path.as_deref(), Some("Research/Web"));
        assert_eq!(save.tags, vec!["rust", "viewer", "local"]);
        assert_eq!(save.aliases, vec!["sample", "example"]);

        let update = update_input_from_viewer(&json!({
            "title": "",
            "category": "",
            "tags": ""
        }));
        assert_eq!(update.title.as_deref(), Some(""));
        assert_eq!(update.folder_path.as_deref(), Some(""));
        assert_eq!(update.tags, Some(Vec::new()));
    }
}
