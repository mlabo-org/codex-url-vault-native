# Codex URL Vault Native

Native replacement source for the `codex-url-vault` plugin. The repository owns four
coherent delivery surfaces:

- `url-vault-core`: SQLite schema, bookmark HTML compatibility, search, snapshots,
  locking, and crash-recoverable canonical export.
- `url-vault-mcp`: task-oriented stdio MCP server used by Codex.
- `url-vault-cli`: machine-readable command-line interface for local operation.
- `Codex URL Vault.app`: SwiftUI browser/editor using the Rust core through an
  operation-specific C ABI.

All surfaces use `CODEX_URL_VAULT_HOME` when set and otherwise share
`~/.codex/url-vault`. The app bundle carries the MCP server and CLI in
`Contents/Resources`.

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
