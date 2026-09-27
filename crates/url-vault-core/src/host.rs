//! Agent host resolution shared by the MCP server, CLI, and localhost viewer.
//!
//! Contract: `AGENT_HOST` set to `codex` or `claude_code` wins; otherwise
//! `CLAUDECODE=1` (exported by Claude Code to MCP servers and tool
//! subprocesses) selects Claude Code; otherwise the host is Codex.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentHost {
    Codex,
    ClaudeCode,
}

impl AgentHost {
    #[must_use]
    pub fn from_env() -> Self {
        Self::resolve(
            std::env::var("AGENT_HOST").ok().as_deref(),
            std::env::var("CLAUDECODE").ok().as_deref(),
        )
    }

    #[must_use]
    pub fn resolve(agent_host: Option<&str>, claudecode: Option<&str>) -> Self {
        match agent_host {
            Some("codex") => Self::Codex,
            Some("claude_code") => Self::ClaudeCode,
            _ if claudecode == Some("1") => Self::ClaudeCode,
            _ => Self::Codex,
        }
    }

    /// Stored identifier used for `opens.opened_by`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
        }
    }

    /// Stored `bookmarks.source_type` for agent-initiated saves.
    #[must_use]
    pub fn capture_source_type(self) -> &'static str {
        match self {
            Self::Codex => "codex_capture",
            Self::ClaudeCode => "claude_code_capture",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AgentHost;

    #[test]
    fn explicit_agent_host_wins_and_unknown_falls_through() {
        assert_eq!(AgentHost::resolve(Some("codex"), Some("1")), AgentHost::Codex);
        assert_eq!(AgentHost::resolve(Some("claude_code"), None), AgentHost::ClaudeCode);
        assert_eq!(AgentHost::resolve(Some("other"), Some("1")), AgentHost::ClaudeCode);
        assert_eq!(AgentHost::resolve(Some("other"), None), AgentHost::Codex);
    }

    #[test]
    fn claudecode_env_selects_claude_code_and_unset_is_codex() {
        assert_eq!(AgentHost::resolve(None, Some("1")), AgentHost::ClaudeCode);
        assert_eq!(AgentHost::resolve(None, None), AgentHost::Codex);
        assert_eq!(AgentHost::ClaudeCode.capture_source_type(), "claude_code_capture");
        assert_eq!(AgentHost::Codex.as_str(), "codex");
    }
}
