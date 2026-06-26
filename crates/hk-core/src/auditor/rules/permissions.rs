use crate::auditor::{AuditInput, AuditRule};
use crate::models::{AuditFinding, ExtensionKind, Permission, Severity, SourceOrigin};

pub struct BroadPermissions;

impl AuditRule for BroadPermissions {
    fn id(&self) -> &str {
        "broad-permissions"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Mcp {
            return vec![];
        }
        let mut findings = Vec::new();
        let all_args = input.mcp_args.join(" ");
        if all_args.contains("--host") && (all_args.contains("*") || all_args.contains("0.0.0.0")) {
            findings.push(AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: "MCP server binds to all interfaces or accepts wildcard hosts".into(),
                location: input.file_path.clone(),
            });
        }
        if let Some(cmd) = &input.mcp_command
            && cmd.contains("filesystem")
            && (all_args.contains("/") && !all_args.contains("/tmp"))
        {
            findings.push(AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: "Filesystem MCP server with broad path access".into(),
                location: input.file_path.clone(),
            });
        }
        findings
    }
}

pub struct SupplyChainRisk;

impl AuditRule for SupplyChainRisk {
    fn id(&self) -> &str {
        "supply-chain"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Mcp {
            return vec![];
        }
        if let Some(cmd) = &input.mcp_command
            && (cmd == "npx" || cmd.ends_with("/npx"))
            && let Some(pkg) = input.mcp_args.iter().find(|a| !a.starts_with('-'))
            && !pkg.starts_with('@')
        {
            return vec![AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: format!(
                    "MCP uses unscoped npm package via npx: {pkg} (typosquatting risk)"
                ),
                location: input.file_path.clone(),
            }];
        }
        vec![]
    }
}

pub struct UnknownSource;

impl AuditRule for UnknownSource {
    fn id(&self) -> &str {
        "unknown-source"
    }

    fn severity(&self) -> Severity {
        Severity::Low
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind == ExtensionKind::Cli {
            return vec![];
        }
        if input.source.origin == SourceOrigin::Local && input.source.url.is_none() {
            vec![AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message:
                    "Extension has no known source — not installed via an agent or tracked in git"
                        .into(),
                location: input.file_path.clone(),
            }]
        } else {
            vec![]
        }
    }
}

pub struct PermissionCombinationRisk;

impl AuditRule for PermissionCombinationRisk {
    fn id(&self) -> &str {
        "permission-combo-risk"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        let has_network = input
            .permissions
            .iter()
            .any(|p| matches!(p, Permission::Network { .. }));
        let has_env = input
            .permissions
            .iter()
            .any(|p| matches!(p, Permission::Env { .. }));
        let has_shell = input
            .permissions
            .iter()
            .any(|p| matches!(p, Permission::Shell { .. }));

        if !has_network && !has_env && !has_shell {
            return vec![];
        }

        let url_re = regex::Regex::new(r"https?://[\w.\-]+").unwrap();
        let shell_re = regex::Regex::new(r"```(?:bash|shell|sh|zsh)").unwrap();
        let env_re =
            regex::Regex::new(r"(?i)(?:process\.env|environ|getenv|\$[A-Z_]+|\.env\b)").unwrap();

        let mut network_line: Option<usize> = None;
        let mut shell_line: Option<usize> = None;
        let mut env_line: Option<usize> = None;

        for (i, line) in input.content.lines().enumerate() {
            if network_line.is_none() && url_re.is_match(line) {
                network_line = Some(i + 1);
            }
            if shell_line.is_none() && shell_re.is_match(line) {
                shell_line = Some(i + 1);
            }
            if env_line.is_none() && env_re.is_match(line) {
                env_line = Some(i + 1);
            }
        }

        let loc = |line: Option<usize>| match line {
            Some(n) => format!("{}:{}", input.file_path, n),
            None => input.file_path.clone(),
        };

        let mut findings = Vec::new();
        if has_network && has_env {
            findings.push(AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: "Has both Network and Env permissions — credential exfiltration risk"
                    .into(),
                location: loc(env_line.or(network_line)),
            });
        }
        if has_shell && has_network {
            findings.push(AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: "Has both Shell and Network permissions — remote code execution risk"
                    .into(),
                location: loc(shell_line.or(network_line)),
            });
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SourceOrigin;

    use crate::auditor::rules::test_support::skill_input;

    #[test]
    fn test_unknown_source() {
        let rule = UnknownSource;
        let input = skill_input("some content");
        assert!(!rule.check(&input).is_empty());
    }

    #[test]
    fn test_unknown_source_git_origin() {
        let rule = UnknownSource;
        let mut input = skill_input("some content");
        input.source.origin = SourceOrigin::Git;
        input.source.url = Some("https://github.com/user/repo".into());
        assert!(rule.check(&input).is_empty());
    }
}
