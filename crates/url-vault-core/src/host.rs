//! Agent host resolution shared by the MCP server, CLI, and localhost viewer.
//!
//! Contract: `AGENT_HOST` set to `codex`, `claude_code`, or `antigravity` wins;
//! otherwise `CLAUDECODE=1` selects Claude Code, `ANTIGRAVITY_AGENT=1` or
//! `ANTIGRAVITY_LS_ADDRESS` selects Antigravity, otherwise the host is Codex.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentHost {
    Codex,
    ClaudeCode,
    Antigravity,
}

impl AgentHost {
    #[must_use]
    pub fn from_env() -> Self {
        Self::resolve_env(
            std::env::var("AGENT_HOST").ok().as_deref(),
            std::env::var("CLAUDECODE").ok().as_deref(),
            std::env::var("ANTIGRAVITY_AGENT").ok().as_deref(),
            std::env::var("ANTIGRAVITY_LS_ADDRESS").ok().as_deref(),
        )
    }

    #[must_use]
    pub fn resolve(agent_host: Option<&str>, claudecode: Option<&str>) -> Self {
        Self::resolve_env(agent_host, claudecode, None, None)
    }

    #[must_use]
    pub fn resolve_env(
        agent_host: Option<&str>,
        claudecode: Option<&str>,
        antigravity_agent: Option<&str>,
        antigravity_ls_address: Option<&str>,
    ) -> Self {
        match agent_host {
            Some("codex") => Self::Codex,
            Some("claude_code") => Self::ClaudeCode,
            Some("antigravity") => Self::Antigravity,
            _ if claudecode == Some("1") => Self::ClaudeCode,
            _ if antigravity_agent == Some("1")
                || antigravity_ls_address.is_some_and(|value| !value.trim().is_empty()) =>
            {
                Self::Antigravity
            }
            _ => Self::Codex,
        }
    }

    /// Stored identifier used for `opens.opened_by`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
            Self::Antigravity => "antigravity",
        }
    }

    /// Stored `bookmarks.source_type` for agent-initiated saves.
    #[must_use]
    pub fn capture_source_type(self) -> &'static str {
        match self {
            Self::Codex => "codex_capture",
            Self::ClaudeCode => "claude_code_capture",
            Self::Antigravity => "antigravity_capture",
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
        assert_eq!(AgentHost::resolve(Some("antigravity"), Some("1")), AgentHost::Antigravity);
        assert_eq!(AgentHost::resolve(Some("other"), Some("1")), AgentHost::ClaudeCode);
        assert_eq!(AgentHost::resolve(Some("other"), None), AgentHost::Codex);
    }

    #[test]
    fn claudecode_and_antigravity_env_select_hosts() {
        assert_eq!(AgentHost::resolve(None, Some("1")), AgentHost::ClaudeCode);
        assert_eq!(AgentHost::resolve(None, None), AgentHost::Codex);
        assert_eq!(AgentHost::resolve_env(None, None, Some("1"), None), AgentHost::Antigravity);
        assert_eq!(
            AgentHost::resolve_env(None, None, None, Some("127.0.0.1:9999")),
            AgentHost::Antigravity
        );
        assert_eq!(AgentHost::ClaudeCode.capture_source_type(), "claude_code_capture");
        assert_eq!(AgentHost::Antigravity.capture_source_type(), "antigravity_capture");
        assert_eq!(AgentHost::Antigravity.as_str(), "antigravity");
        assert_eq!(AgentHost::Codex.as_str(), "codex");
    }
}
