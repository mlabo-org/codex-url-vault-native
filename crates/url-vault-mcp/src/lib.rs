use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use url_vault_core::{AgentHost, Vault, VaultError};
use url_vault_native_host::{SaveCurrentBrowserPageInput, request_save_current_browser_page};
use url_vault_viewer::start_iab_viewer;

pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

pub fn serve<R: BufRead, W: Write>(reader: R, mut writer: W) -> io::Result<()> {
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle_input(&message),
            Err(error) => Some(rpc_error(
                Value::Null,
                -32700,
                format!("parse error: {error}"),
            )),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut writer, &response).map_err(io::Error::other)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
    }
    Ok(())
}

pub fn handle_input(message: &Value) -> Option<Value> {
    if let Some(batch) = message.as_array() {
        let responses = batch.iter().filter_map(handle_request).collect::<Vec<_>>();
        return (!responses.is_empty()).then_some(Value::Array(responses));
    }
    handle_request(message)
}

fn handle_request(message: &Value) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let host = AgentHost::from_env();
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match method {
        "initialize" => Some(rpc_result(
            id,
            initialize_result(message.get("params"), host),
        )),
        "ping" => Some(rpc_result(id, json!({}))),
        "tools/list" => Some(rpc_result(id, json!({ "tools": tool_records(host) }))),
        "tools/call" => Some(handle_tool_call(id, message.get("params"), host)),
        _ => Some(rpc_error(id, -32601, format!("unknown method: {method}"))),
    }
}

fn initialize_result(_params: Option<&Value>, host: AgentHost) -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "codex-url-vault",
            "title": "Codex URL Vault",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": format!(
            "Use task-oriented URL Vault tools. Use save_current_browser_page for requests such as 'save this URL' that refer to the currently focused Brave or Chrome page; pass browser only when the user explicitly identifies it, and never infer the URL from the screen. Suggest before opening vague requests, preserve snapshots only from explicit UTF-8 content, use show_vault for the independent native app, and use show_iab_vault when the user wants the card-based Vault inside {}.",
            in_app_browser(host)
        )
    })
}

fn handle_tool_call(id: Value, params: Option<&Value>, host: AgentHost) -> Value {
    let name = params
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = params
        .and_then(|value| value.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let result = call_tool(name, &arguments, host)
        .map(tool_success)
        .unwrap_or_else(tool_failure);
    rpc_result(id, result)
}

fn call_tool(name: &str, arguments: &Value, host: AgentHost) -> Result<Value, VaultError> {
    if name == "show_vault" {
        let app_path = optional_string(arguments, "appPath").map(PathBuf::from);
        return Ok(json!({ "appPath": Vault::show_vault(app_path.as_deref())? }));
    }
    if name == "show_iab_vault" {
        let viewer = start_iab_viewer(host).map_err(|error| {
            VaultError::InvalidInput(format!("{} viewer: {error}", in_app_browser(host)))
        })?;
        return Ok(serde_json::to_value(viewer)?);
    }
    let vault = Vault::from_env()?;
    match name {
        "search_urls" => Ok(serde_json::to_value(vault.search_urls(
            required_string(arguments, "query")?,
            optional_usize(arguments, "limit").unwrap_or(10),
        )?)?),
        "suggest_urls" => Ok(serde_json::to_value(vault.suggest_urls(
            required_string(arguments, "query")?,
            optional_usize(arguments, "limit").unwrap_or(5),
        )?)?),
        "get_url" => Ok(serde_json::to_value(
            vault.get_url(required_string(arguments, "target")?)?,
        )?),
        "list_urls" => Ok(serde_json::to_value(vault.list_urls(
            optional_string(arguments, "category"),
            optional_usize(arguments, "limit"),
        )?)?),
        "list_categories" => Ok(serde_json::to_value(vault.list_categories()?)?),
        "get_snapshot" => Ok(serde_json::to_value(
            vault.get_snapshot(required_string(arguments, "target")?)?,
        )?),
        "verify_snapshots" => Ok(serde_json::to_value(
            vault.verify_snapshots(optional_string(arguments, "target"))?,
        )?),
        "save_url" => Ok(serde_json::to_value(
            vault.save_url(from_value(arguments)?)?,
        )?),
        "save_current_browser_page" => request_save_current_browser_page(
            &vault,
            from_value::<SaveCurrentBrowserPageInput>(arguments)?,
        )
        .map_err(|error| VaultError::InvalidInput(error.to_string())),
        "update_url" => {
            let target = required_string(arguments, "target")?;
            let input = arguments.get("changes").ok_or_else(|| {
                VaultError::InvalidInput("update_url requires changes".to_owned())
            })?;
            Ok(serde_json::to_value(
                vault.update_url(target, from_value(input)?)?,
            )?)
        }
        "move_url" => Ok(serde_json::to_value(vault.move_url(
            required_string(arguments, "target")?,
            required_string(arguments, "category")?,
        )?)?),
        "archive_url" => Ok(serde_json::to_value(
            vault.archive_url(required_string(arguments, "target")?)?,
        )?),
        "create_category" => Ok(serde_json::to_value(vault.create_category(
            required_string(arguments, "path")?,
            optional_string(arguments, "label"),
            optional_string(arguments, "note"),
        )?)?),
        "rename_category" => Ok(serde_json::to_value(vault.rename_category(
            required_string(arguments, "oldPath")?,
            required_string(arguments, "newPath")?,
        )?)?),
        "archive_category" => Ok(serde_json::to_value(vault.archive_category(
            required_string(arguments, "path")?,
            optional_string(arguments, "moveTo").unwrap_or_default(),
        )?)?),
        "preview_bookmark_import" => Ok(serde_json::to_value(vault.preview_bookmark_import(
            required_string(arguments, "html")?,
            required_string(arguments, "fileName")?,
            optional_bool(arguments, "stripCommonRoot").unwrap_or(false),
        )?)?),
        "apply_bookmark_import" => Ok(serde_json::to_value(
            vault.apply_bookmark_import(from_value(arguments)?)?,
        )?),
        "save_snapshot" => Ok(serde_json::to_value(
            vault.save_snapshot(from_value(arguments)?)?,
        )?),
        "open_url" => Ok(serde_json::to_value(vault.open_url(
            required_string(arguments, "target")?,
            optional_string(arguments, "browser"),
            optional_bool(arguments, "dryRun").unwrap_or(false),
            optional_string(arguments, "openedBy").unwrap_or(host.as_str()),
            optional_string(arguments, "context"),
        )?)?),
        _ => Err(VaultError::InvalidInput(format!("unknown tool: {name}"))),
    }
}

/// Agent-facing name of the host's in-app browser that opens the viewer URL.
fn in_app_browser(host: AgentHost) -> &'static str {
    match host {
        AgentHost::Codex => "Codex Browser/IAB",
        AgentHost::ClaudeCode => "the Claude Code built-in browser pane",
    }
}

fn show_iab_vault_description(host: AgentHost) -> &'static str {
    match host {
        AgentHost::Codex => {
            "Start or reuse the authenticated localhost card-based Vault viewer and return its viewerUrl. Open that URL with Codex Browser/IAB; this tool does not navigate the browser itself."
        }
        AgentHost::ClaudeCode => {
            "Start or reuse the authenticated localhost card-based Vault viewer and return its viewerUrl. Open that URL in the Claude Code built-in browser pane with mcp__Claude_Browser__preview_start (url) or mcp__Claude_Browser__navigate; this tool does not navigate the browser itself."
        }
    }
}

fn tool_records(host: AgentHost) -> Vec<Value> {
    vec![
        tool(
            "search_urls",
            "Search active saved URLs and verified snapshot text. Returns ranked candidates without opening anything.",
            schema(
                &["query"],
                &[("query", string()), ("limit", integer(1, 100))],
            ),
            read_annotations("Search saved URLs"),
        ),
        tool(
            "suggest_urls",
            "Resolve a vague conversational request into a short ranked candidate list and an open_directly decision.",
            schema(
                &["query"],
                &[("query", string()), ("limit", integer(1, 20))],
            ),
            read_annotations("Suggest saved URLs"),
        ),
        tool(
            "get_url",
            "Get one active bookmark by stable ID, exact URL, alias, title, or clear search target.",
            schema(&["target"], &[("target", string())]),
            read_annotations("Get a saved URL"),
        ),
        tool(
            "list_urls",
            "List active bookmarks, optionally within one category.",
            schema(&[], &[("category", string()), ("limit", integer(1, 1000))]),
            read_annotations("List saved URLs"),
        ),
        tool(
            "list_categories",
            "List active Vault categories with bookmark counts.",
            schema(&[], &[]),
            read_annotations("List Vault categories"),
        ),
        tool(
            "get_snapshot",
            "Verify and return the explicitly preserved local UTF-8 snapshot for one saved URL. Does not fetch the web.",
            schema(&["target"], &[("target", string())]),
            read_annotations("Get preserved content"),
        ),
        tool(
            "verify_snapshots",
            "Verify one or all active snapshot references for containment, presence, hash, byte count, and UTF-8 validity.",
            schema(&[], &[("target", string())]),
            write_annotations("Verify preserved content", true, false),
        ),
        tool(
            "save_url",
            "Save or upsert one URL with title, notes, tags, aliases, intents, category, project, and browser metadata.",
            save_url_schema(),
            write_annotations("Save a URL", false, false),
        ),
        tool(
            "save_current_browser_page",
            "Save the exact current Brave or Chrome http/https page when the user says 'save this URL', 'save this page', or asks to capture the current browser tab. Uses the selected bundled extension bridge for URL and title; never infers them from screen content.",
            save_current_browser_page_schema(),
            action_annotations("Save the current browser page"),
        ),
        tool(
            "update_url",
            "Update one bookmark. A changed URL creates a replacement identity and marks the old record replaced.",
            schema(
                &["target", "changes"],
                &[("target", string()), ("changes", update_url_schema())],
            ),
            write_annotations("Update a saved URL", false, false),
        ),
        tool(
            "move_url",
            "Move one active bookmark to a category without changing its URL identity.",
            schema(
                &["target", "category"],
                &[("target", string()), ("category", string())],
            ),
            write_annotations("Move a saved URL", true, false),
        ),
        tool(
            "archive_url",
            "Soft-archive one active bookmark. The record is retained and ordinary searches stop returning it.",
            schema(&["target"], &[("target", string())]),
            write_annotations("Archive a saved URL", true, true),
        ),
        tool(
            "create_category",
            "Create or reactivate a Vault category.",
            schema(
                &["path"],
                &[("path", string()), ("label", string()), ("note", string())],
            ),
            write_annotations("Create a Vault category", true, false),
        ),
        tool(
            "rename_category",
            "Rename a category and move its active bookmarks while preserving import history.",
            schema(
                &["oldPath", "newPath"],
                &[("oldPath", string()), ("newPath", string())],
            ),
            write_annotations("Rename a Vault category", false, false),
        ),
        tool(
            "archive_category",
            "Archive a category and move contained bookmarks to Unfiled or a specified destination.",
            schema(&["path"], &[("path", string()), ("moveTo", string())]),
            write_annotations("Archive a Vault category", true, true),
        ),
        tool(
            "preview_bookmark_import",
            "Parse local Netscape bookmark HTML without mutation and return counts plus the SHA-256 required by apply.",
            schema(
                &["html", "fileName"],
                &[
                    ("html", string()),
                    ("fileName", string()),
                    ("stripCommonRoot", boolean()),
                ],
            ),
            read_annotations("Preview a bookmark import"),
        ),
        tool(
            "apply_bookmark_import",
            "Apply an add, merge, or explicitly confirmed reset import only when HTML matches the preview SHA-256.",
            import_schema(),
            write_annotations("Apply a bookmark import", false, true),
        ),
        tool(
            "save_snapshot",
            "Preserve explicit UTF-8 text or Markdown for an existing bookmark. URL Vault never fetches the page.",
            snapshot_schema(),
            write_annotations("Save preserved content", false, false),
        ),
        tool(
            "open_url",
            "Resolve and safely open a saved http/https URL in the configured macOS browser. Vague requests require a clear winner.",
            schema(
                &["target"],
                &[
                    ("target", string()),
                    (
                        "browser",
                        enum_string(&["default", "brave", "chrome", "firefox", "safari"]),
                    ),
                    ("dryRun", boolean()),
                    ("openedBy", string()),
                    ("context", string()),
                ],
            ),
            action_annotations("Open a saved URL"),
        ),
        tool(
            "show_vault",
            "Launch or activate the independent native Codex URL Vault macOS app.",
            schema(&[], &[("appPath", string())]),
            action_annotations("Show Codex URL Vault"),
        ),
        tool(
            "show_iab_vault",
            show_iab_vault_description(host),
            schema(&[], &[]),
            action_annotations(match host {
                AgentHost::Codex => "Show URL Vault in Codex Browser",
                AgentHost::ClaudeCode => "Show URL Vault in Claude Code browser pane",
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value, annotations: Value) -> Value {
    let mut input_schema = input_schema;
    if let Some(object) = input_schema.as_object_mut() {
        object.insert(
            "description".to_owned(),
            Value::String(format!("Arguments for the {name} URL Vault operation.")),
        );
    }
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": annotations
    })
}

fn schema(required: &[&str], properties: &[(&str, Value)]) -> Value {
    let properties = properties
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

fn save_url_schema() -> Value {
    schema(
        &["url"],
        &[
            ("url", string()),
            ("title", string()),
            ("description", string()),
            ("note", string()),
            ("tags", string_array()),
            ("aliases", string_array()),
            ("intents", string_array()),
            ("source_type", string()),
            ("source_browser", string()),
            ("source_profile", string()),
            ("folder_path", string()),
            ("preferred_browser", string()),
            ("project", string()),
            ("status", string()),
        ],
    )
}

fn save_current_browser_page_schema() -> Value {
    schema(
        &[],
        &[
            (
                "browser",
                json!({ "type": "string", "enum": ["brave", "chrome"] }),
            ),
            ("folder_path", string()),
            ("tags", string_array()),
            ("note", string()),
        ],
    )
}

fn update_url_schema() -> Value {
    schema(
        &[],
        &[
            ("url", string()),
            ("title", string()),
            ("description", string()),
            ("note", string()),
            ("tags", string_array()),
            ("aliases", string_array()),
            ("intents", string_array()),
            ("folder_path", string()),
            ("preferred_browser", string()),
            ("project", string()),
        ],
    )
}

fn import_schema() -> Value {
    schema(
        &["html", "file_name", "source_browser", "expected_sha256"],
        &[
            ("html", string()),
            ("file_name", string()),
            ("source_browser", string()),
            ("source_profile", string()),
            ("preferred_browser", string()),
            ("mode", enum_string(&["add", "merge", "reset"])),
            ("strip_common_root", boolean()),
            ("expected_sha256", string()),
            ("confirm_reset", boolean()),
        ],
    )
}

fn snapshot_schema() -> Value {
    schema(
        &["target", "content"],
        &[
            ("target", string()),
            ("content", string()),
            ("kind", enum_string(&["semantic", "verbatim"])),
            ("title", string()),
            ("captured_at", string()),
        ],
    )
}

fn string() -> Value {
    json!({ "type": "string" })
}

fn integer(minimum: usize, maximum: usize) -> Value {
    json!({ "type": "integer", "minimum": minimum, "maximum": maximum })
}

fn boolean() -> Value {
    json!({ "type": "boolean" })
}

fn string_array() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

fn enum_string(values: &[&str]) -> Value {
    json!({ "type": "string", "enum": values })
}

fn read_annotations(title: &str) -> Value {
    json!({
        "title": title,
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
    })
}

fn write_annotations(title: &str, idempotent: bool, destructive: bool) -> Value {
    json!({
        "title": title,
        "readOnlyHint": false,
        "destructiveHint": destructive,
        "idempotentHint": idempotent,
        "openWorldHint": false
    })
}

fn action_annotations(title: &str) -> Value {
    json!({
        "title": title,
        "readOnlyHint": false,
        "destructiveHint": false,
        "idempotentHint": false,
        "openWorldHint": true
    })
}

fn tool_success(value: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
        "structuredContent": value,
        "isError": false
    })
}

fn tool_failure(error: VaultError) -> Value {
    json!({
        "content": [{ "type": "text", "text": error.to_string() }],
        "isError": true
    })
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn required_string<'a>(value: &'a Value, name: &str) -> Result<&'a str, VaultError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| VaultError::InvalidInput(format!("{name} is required")))
}

fn optional_string<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(Value::as_str)
}

fn optional_usize(value: &Value, name: &str) -> Option<usize> {
    value
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn optional_bool(value: &Value, name: &str) -> Option<bool> {
    value.get(name).and_then(Value::as_bool)
}

fn from_value<T: DeserializeOwned>(value: &Value) -> Result<T, VaultError> {
    serde_json::from_value(value.clone()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_task_oriented_tools_and_annotations() {
        let tools = tool_records(AgentHost::Codex);
        assert_eq!(tools.len(), 21);
        assert!(tools.iter().any(|tool| tool["name"] == "show_vault"));
        assert!(tools.iter().any(|tool| tool["name"] == "show_iab_vault"));
        let current_page = tools
            .iter()
            .find(|tool| tool["name"] == "save_current_browser_page")
            .expect("current browser page tool");
        assert!(
            current_page["description"]
                .as_str()
                .unwrap()
                .contains("save this URL")
        );
        assert_eq!(current_page["inputSchema"]["required"], json!([]));
        assert_eq!(
            current_page["inputSchema"]["properties"]["browser"]["enum"],
            json!(["brave", "chrome"])
        );
        assert_eq!(current_page["annotations"]["openWorldHint"], true);
        assert_eq!(
            tools
                .iter()
                .find(|tool| tool["name"] == "archive_url")
                .unwrap()["annotations"]["destructiveHint"],
            true
        );
    }

    #[test]
    fn initializes_without_starting_http() {
        let response = handle_input(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": MCP_PROTOCOL_VERSION }
        }))
        .unwrap();
        assert_eq!(response["result"]["serverInfo"]["name"], "codex-url-vault");
        assert_eq!(response["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
    }

    #[test]
    fn host_selects_agent_facing_browser_wording() {
        let show_iab = |host| {
            tool_records(host)
                .into_iter()
                .find(|tool| tool["name"] == "show_iab_vault")
                .unwrap()["description"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let codex = initialize_result(None, AgentHost::Codex);
        assert!(
            codex["instructions"]
                .as_str()
                .unwrap()
                .ends_with("inside Codex Browser/IAB.")
        );
        assert!(show_iab(AgentHost::Codex).contains("Codex Browser/IAB"));

        let claude = initialize_result(None, AgentHost::ClaudeCode);
        let instructions = claude["instructions"].as_str().unwrap();
        assert!(instructions.ends_with("inside the Claude Code built-in browser pane."));
        assert!(!instructions.contains("IAB"));
        let description = show_iab(AgentHost::ClaudeCode);
        assert!(description.contains("mcp__Claude_Browser__preview_start"));
        assert!(!description.contains("Codex"));
    }
}
