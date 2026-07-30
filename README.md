# Codex URL Vault Native

Native replacement source for the `codex-url-vault` plugin. The repository owns seven
coherent delivery surfaces:

- `url-vault-core`: SQLite schema, bookmark HTML compatibility, search, snapshots,
  locking, and crash-recoverable canonical export.
- `url-vault-mcp`: task-oriented stdio MCP server used by Codex.
- `url-vault-viewer`: authenticated ephemeral localhost Viewer for Codex Browser/IAB,
  using the compatible responsive card UI.
- `url-vault-cli`: machine-readable command-line interface for local operation.
- `Codex URL Vault.app`: SwiftUI browser/editor using the Rust core through an
  operation-specific C ABI.
- `url-vault-native-host`: Brave Native Messaging adapter over the same Rust core.
- `extension/brave`: Bitwarden-style current-tab capture popup and context menu.

All surfaces use `CODEX_URL_VAULT_HOME` when set and otherwise share
`~/.codex/url-vault`. The app bundle carries the MCP server and CLI in
`Contents/Resources`.

`show_vault` opens the independent SwiftUI app. `show_iab_vault` starts or reuses the
Viewer inside the MCP process and returns a session URL for Codex Browser/IAB. The
Viewer uses a random loopback port and session token; it does not use the retired
Python server or fixed port.

The Brave extension uses stable ID `fhijpanhohijhimeiajdgfhcpipbhkkh`. Its popup
prefills the active tab and accepts category, tags, and a note. The context menu saves
the current page or selected link with the last-used category and tags. The extension
holds one bidirectional `runtime.connectNative` port to
`com.suzukimakoto.codex_url_vault`; popup and context-menu requests share that port.
The extension never opens SQLite or stdio MCP directly.

The MCP `save_current_brave_page` tool uses the same connection for deictic requests
such as “save this URL.” The MCP sends one request through the user-only Unix socket
at `<vault-home>/.brave-current-page.sock`. The Native Messaging Host asks the
extension for the active tab in the last-focused, currently foreground Brave window,
then saves the returned exact URL and title through the shared Rust core. The
extension requires the `tabs` permission because this route has no Brave action click
that could grant temporary `activeTab` access. Only stable `http` and `https` tabs are
accepted; an unfocused Brave window, a changed target tab, or a disconnected bridge
returns an error without URL inference or fallback capture.

Source locations:

```text
extension/brave/
extension/native-host/com.suzukimakoto.codex_url_vault.json
```

Validate the extension contract with:

```bash
node scripts/validate-brave-extension.mjs
```

The native host can render a registration manifest for a chosen absolute binary path:

```bash
url-vault-native-host --print-brave-manifest /absolute/path/to/url-vault-native-host
```

Brave unpacked-extension loading, Native Messaging registration, and runtime
activation are separate operational steps. On this macOS Brave setup, install the
same host manifest in Brave's browser-wide and active-profile
`NativeMessagingHosts` directories and in the Chromium compatibility lookup at
`~/Library/Application Support/Google/Chrome/NativeMessagingHosts/`, then restart
Brave. The compatibility manifest lets Brave discover the host; it does not install
the extension in Chrome.

After an extension source update, reload the unpacked extension at
`brave://extensions` so its service worker and permission set are current. A newly
added `tabs` permission may require explicit acceptance in Brave.

## Build

```bash
./scripts/build-release.sh
```

The runnable source artifact is emitted at:

```text
dist/Codex URL Vault.app
```

Build, validation, plugin refresh, application installation, and replacement of the
active plugin are separate stages. This repository performs none of the latter three
implicitly.

## Parent orchestration contract

The MCP server exposes independent task operations rather than one opaque workflow.
Read-only search/list/inspection work can be assigned independently when that
materially helps a parent task. The parent retains user intent, mutation approval,
result integration, and release decisions.

Each operation returns a structured result or a concrete error. The Vault directory,
SQLite database, canonical bookmark HTML, and snapshot paths are file-backed artifacts;
the Rust workspace tests and a temporary-home MCP/CLI smoke are the validation entry
points. A missing app binary, invalid Vault path, failed checksum, unsafe snapshot
path, or unresolved ambiguous open request is a stop condition rather than a fallback
route.
