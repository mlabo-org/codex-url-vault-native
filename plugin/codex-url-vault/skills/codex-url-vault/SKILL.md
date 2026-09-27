---
name: codex-url-vault
description: "Operate Codex URL Vault: save this URL or the current Brave or Chrome page, find and organize saved links, open the native macOS Vault or the in-agent viewer (Codex Browser/IAB or Claude Code browser pane), import bookmarks, and preserve supplied text. Route through task-oriented MCP tools."
---

# Codex URL Vault

This `SKILL.md` is the local execution contract for this skill when the skill is selected.
The host agent (Codex or Claude Code) must treat this file's trigger assumptions, workflow, tool boundaries, file
boundaries, and output shape as binding instructions within this skill's scope.
This file does not override system instructions, developer instructions, explicit user
requests, applicable `AGENTS.md` or `CLAUDE.md` files, or more specific local execution contracts.

## Trigger and primary route

Use this skill when the user asks to save, find, list, inspect, organize, import, open,
or show URLs in Codex URL Vault; says “save this URL,” “save this page,” or otherwise
refers to the current Brave or Chrome page; asks to display the Vault inside the agent's browser (Codex Browser/IAB or the Claude Code
built-in browser pane);
or asks for explicitly preserved text associated with a saved URL.

First use the plugin's `codex-url-vault` MCP tools. Do not replace those tools with
shell commands, direct SQLite changes, or direct edits to Vault files. A bare request
to show or open “the Vault” means `show_vault`. An explicit request for IAB, the browser pane, Codex内, Claude内,
or a browser-contained Vault means `show_iab_vault`; open the returned localhost
`viewerUrl` in the host's browser. In Codex: Codex Browser/IAB. In Claude Code: the
built-in browser pane with `mcp__Claude_Browser__preview_start` (or
`mcp__Claude_Browser__navigate` when the pane is already open). Do not substitute `file://`, shell launch, or
the retired viewer runtime.

Do not select this skill for general web research, arbitrary browser history, or
content acquisition. Snapshot preservation stores text supplied by the caller. The
viewer's Reader Preview may fetch a page for temporary display, but it does not preserve
that content as a snapshot.

For “this URL,” “this page,” or another explicit current-browser-page reference, call
`save_current_browser_page`. It obtains the exact URL and title through the selected
Brave or Chrome extension, Native Messaging Host, and local bridge before saving.
Pass `browser: "brave"` or `browser: "chrome"` only when the user explicitly identifies
the browser; otherwise omit it and use the extension selected during installation. Do
not ask the user to copy the URL, infer it from visible text, use AppShot as a URL
source, or reopen the page in another managed browser tab. Use `save_url` only when
the caller has already supplied the URL.

## Operation routing

- Search or vague recall: call `search_urls` or `suggest_urls`. Open only after the
  result is unambiguous or the user selects a candidate.
- Browse saved material: call `list_urls`, `list_categories`, `get_url`, or
  `get_snapshot`.
- Open a link or the independent app: call `open_url` or `show_vault`.
- Show the card-based Vault inside the agent: call `show_iab_vault`, then open its
  returned `viewerUrl` in Codex Browser/IAB (Codex) or the built-in browser pane
  (Claude Code).
- Save the current Brave or Chrome page: call `save_current_browser_page`; pass an
  explicitly requested browser, category, tags, or note when present.
- Save an explicit URL or organize existing records: call `save_url`, `update_url`,
  `move_url`, `archive_url`, or the category tools that directly match the request.
- Import browser bookmark HTML: call `preview_bookmark_import`, preserve its checksum,
  then call `apply_bookmark_import` with the user's selected import mode. A reset
  requires the user's explicit reset intent.
- Preserve supplied UTF-8 text: call `save_snapshot`; use `verify_snapshots` only when
  verification was requested or a concrete snapshot failure is being diagnosed.

Use stable bookmark IDs returned by read tools when a later mutation targets a specific
record. Report the resulting bookmark, category, import counts, snapshot status, or
opened URL rather than exposing internal database details.

## Parent and worker boundary

The parent task retains user intent, mutation decisions, integration, and final
reporting. Independent read-only searches may be delegated as bounded work when this
materially helps; each worker returns only the query, matched IDs, and concrete
blockers. Do not delegate an ordinary single-tool request or let a worker broaden a
mutation.

## Stop conditions

Stop the affected action and report the exact error when the MCP server is unavailable,
the native app is missing, the selected browser bridge is disconnected, the selected browser is not foreground,
the current tab changes during capture, the current page is not `http` or `https`, a
bookmark target remains ambiguous, an import checksum no longer matches its preview,
the in-agent viewer cannot start, or the core rejects a snapshot path or payload. Do not
substitute another implementation or modify runtime files to bypass the failure.
