// Devin Desktop compatibility adapter.
//
// Devin Desktop is the renamed Windsurf IDE. New workspace files prefer
// `.devin/`, while `.windsurf/` and `.windsurfrules` remain supported as
// legacy fallbacks. Keep the internal adapter name as "windsurf" so existing
// HarnessKit rows and settings remain stable.
//
// Hook reference:     https://docs.windsurf.com/windsurf/cascade/hooks
// Config file:        ~/.config/devin/hooks.json (global), .devin/hooks.json (project)
//                     ~/.codeium/windsurf/hooks.json and .windsurf/hooks.json (legacy)
// Format:             JSON, top-level key "hooks", sub-keys: command (or powershell)
//
// Workflow reference: https://docs.windsurf.com/windsurf/cascade/workflows
// Files:              ~/.config/devin/global_workflows/*.md (global)
//                     .devin/workflows/*.md (project)
//                     legacy ~/.codeium/windsurf/global_workflows and .windsurf/workflows
//
// Ignore reference:   https://docs.windsurf.com/context-awareness/windsurf-ignore
// File:               .codeiumignore / .windsurfignore (project root)

use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry, ProjectMarker, RemoteMcpSchema};
use std::path::{Path, PathBuf};

pub struct WindsurfAdapter {
    home: PathBuf,
}

impl Default for WindsurfAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WindsurfAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    fn devin_base_dir(&self) -> PathBuf {
        self.home.join(".config").join("devin")
    }

    fn legacy_base_dir(&self) -> PathBuf {
        self.home.join(".codeium").join("windsurf")
    }

    fn mcp_config_candidates(&self) -> Vec<PathBuf> {
        vec![
            self.devin_base_dir().join("mcp_config.json"),
            self.legacy_base_dir().join("mcp_config.json"),
            self.home.join(".codeium").join("mcp_config.json"),
        ]
    }

    fn hook_config_candidates(&self) -> Vec<PathBuf> {
        vec![
            self.devin_base_dir().join("hooks.json"),
            self.legacy_base_dir().join("hooks.json"),
        ]
    }

    fn first_existing_or_preferred(paths: Vec<PathBuf>) -> PathBuf {
        paths
            .iter()
            .find(|path| path.exists())
            .cloned()
            .unwrap_or_else(|| paths.into_iter().next().unwrap_or_default())
    }

    fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for path in paths {
            let key = path
                .canonicalize()
                .unwrap_or_else(|_| path.clone())
                .to_string_lossy()
                .to_string();
            if seen.insert(key) {
                deduped.push(path);
            }
        }
        deduped
    }

    fn markdown_files_in_dirs(dirs: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            files.extend(
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().is_some_and(|ext| ext == "md")),
            );
        }
        Self::dedupe_paths(files)
    }

    fn read_json(&self, path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for WindsurfAdapter {
    fn hook_format(&self) -> HookFormat {
        HookFormat::Windsurf
    }

    fn name(&self) -> &str {
        "windsurf"
    }

    fn needs_path_injection(&self) -> bool {
        true
    }

    fn base_dir(&self) -> PathBuf {
        self.devin_base_dir()
    }

    fn detect(&self) -> bool {
        self.devin_base_dir().exists()
            || self.legacy_base_dir().exists()
            || self.home.join(".codeium").exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        Self::dedupe_paths(vec![
            self.devin_base_dir().join("skills"),
            self.legacy_base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ])
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".devin/skills".into(), ".windsurf/skills".into()]
    }

    fn mcp_config_path(&self) -> PathBuf {
        Self::first_existing_or_preferred(self.mcp_config_candidates())
    }

    fn hook_config_path(&self) -> PathBuf {
        Self::first_existing_or_preferred(self.hook_config_candidates())
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        let mut paths: Vec<PathBuf> = self
            .mcp_config_candidates()
            .into_iter()
            .filter(|path| path.exists())
            .collect();
        if paths.is_empty() {
            paths.push(self.mcp_config_path());
        }

        let mut seen = std::collections::HashSet::new();
        let mut servers = Vec::new();
        for path in paths {
            for server in self.read_mcp_servers_from(&path) {
                if seen.insert(server.name.clone()) {
                    servers.push(server);
                }
            }
        }
        servers
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(config) = self.read_json(path) else {
            return vec![];
        };
        let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) else {
            return vec![];
        };

        servers
            .iter()
            .map(|(name, val)| {
                // Remote entries: {serverUrl, headers} — protocol auto-detected.
                let (transport, url) = super::parse_plain_url(val, "serverUrl");
                McpServerEntry {
                    name: name.clone(),
                    command: val
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .into(),
                    args: super::json_string_vec(val, "args"),
                    env: super::json_string_map(val, "env"),
                    transport,
                    url,
                    headers: super::json_string_map(val, "headers"),
                    // Windsurf's MCP schema has no agent-native disable concept.
                    enabled: true,
                }
            })
            .collect()
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::ServerUrl
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_windsurf(event)
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        let mut paths: Vec<PathBuf> = self
            .hook_config_candidates()
            .into_iter()
            .filter(|path| path.exists())
            .collect();
        if paths.is_empty() {
            paths.push(self.hook_config_path());
        }

        let mut seen = std::collections::HashSet::new();
        let mut hooks = Vec::new();
        for path in paths {
            for hook in self.read_hooks_from(&path) {
                let key = format!("{}:{}", hook.event, hook.command);
                if seen.insert(key) {
                    hooks.push(hook);
                }
            }
        }
        hooks
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        let Some(config) = self.read_json(path) else {
            return vec![];
        };
        let Some(hooks) = config.get("hooks").and_then(|v| v.as_object()) else {
            return vec![];
        };

        let mut entries = Vec::new();
        for (event, hook_list) in hooks {
            let Some(arr) = hook_list.as_array() else {
                continue;
            };
            for hook in arr {
                let command = hook
                    .get("command")
                    .and_then(|v| v.as_str())
                    .or_else(|| hook.get("powershell").and_then(|v| v.as_str()));
                if let Some(command) = command {
                    entries.push(HookEntry {
                        event: event.clone(),
                        matcher: None,
                        command: command.to_string(),
                        enabled: true,
                    });
                }
            }
        }
        entries
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        Self::dedupe_paths(vec![
            self.devin_base_dir().join("global_rules.md"),
            self.legacy_base_dir().join("global_rules.md"),
        ])
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        Self::markdown_files_in_dirs(vec![
            self.devin_base_dir().join("memories"),
            self.legacy_base_dir().join("memories"),
        ])
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self
            .mcp_config_candidates()
            .into_iter()
            .filter(|path| path.exists())
            .collect();
        if files.is_empty() {
            files.push(self.mcp_config_path());
        }

        let mut hooks: Vec<PathBuf> = self
            .hook_config_candidates()
            .into_iter()
            .filter(|path| path.exists())
            .collect();
        if hooks.is_empty() {
            hooks.push(self.hook_config_path());
        }
        files.extend(hooks);
        Self::dedupe_paths(files)
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        vec![
            ProjectMarker::Dir(".devin"),
            ProjectMarker::Dir(".windsurf"),
            ProjectMarker::File(".windsurfrules"),
            ProjectMarker::File(".codeiumignore"),
            ProjectMarker::File(".windsurfignore"),
        ]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            ".devin/rules/**/*.md".into(),
            ".windsurfrules".into(),
            ".windsurf/rules/**/*.md".into(),
        ]
    }

    fn project_memory_patterns(&self) -> Vec<String> {
        vec![
            ".devin/memories/*.md".into(),
            ".windsurf/memories/*.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".devin/hooks.json".into(), ".windsurf/hooks.json".into()]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".codeiumignore".into(), ".windsurfignore".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        // Windsurf MCP is global-only: the official MCP doc documents a
        // single config at `~/.codeium/windsurf/mcp_config.json` and never
        // mentions a workspace path — unlike the skills and hooks docs on
        // the same site, which explicitly scope `.windsurf/skills/` and
        // `.windsurf/hooks.json` to the workspace. Third-party guides
        // confirm ("Windsurf doesn't load a project-scoped copy").
        // Source: https://docs.windsurf.com/windsurf/cascade/mcp
        None
    }

    fn project_hook_config_relpath(&self) -> Option<String> {
        Some(".devin/hooks.json".into())
    }

    fn hook_config_paths_for(&self, scope: &crate::models::ConfigScope) -> Vec<PathBuf> {
        match scope {
            crate::models::ConfigScope::Global => vec![self.hook_config_path()],
            crate::models::ConfigScope::Project { path, .. } => {
                let project = Path::new(path);
                vec![
                    project.join(".devin").join("hooks.json"),
                    project.join(".windsurf").join("hooks.json"),
                ]
            }
        }
    }

    fn global_workflow_files(&self) -> Vec<PathBuf> {
        Self::markdown_files_in_dirs(vec![
            self.devin_base_dir().join("global_workflows"),
            self.legacy_base_dir().join("global_workflows"),
        ])
    }

    fn project_workflow_patterns(&self) -> Vec<String> {
        vec![
            ".devin/workflows/*.md".into(),
            ".windsurf/workflows/*.md".into(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::models::ConfigScope;

    use super::super::{AgentAdapter, McpTransport};
    use super::*;

    #[test]
    fn read_mcp_servers_parses_server_url_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("mcp_config.json");
        std::fs::write(
            &config,
            r#"{"mcpServers":{
                "remote":{"serverUrl":"https://example.com/mcp","headers":{"Authorization":"Bearer t"}},
                "fs":{"command":"npx","args":["-y","srv"]}
            }}"#,
        )
        .unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers_from(&config);
        let remote = servers.iter().find(|s| s.name == "remote").unwrap();
        assert_eq!(remote.transport, McpTransport::Http);
        assert_eq!(remote.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(remote.command, "");
        assert_eq!(remote.headers["Authorization"], "Bearer t");
        let fs = servers.iter().find(|s| s.name == "fs").unwrap();
        assert_eq!(fs.transport, McpTransport::Stdio);
    }

    #[test]
    fn detect_supports_devin_and_legacy_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());

        std::fs::create_dir_all(tmp.path().join(".config/devin")).unwrap();
        assert!(adapter.detect());

        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join(".codeium/windsurf")).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn base_dir_prefers_devin_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        assert_eq!(adapter.base_dir(), tmp.path().join(".config/devin"));
    }

    #[test]
    fn mcp_config_path_prefers_devin_then_legacy_then_codeium_root() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        assert_eq!(
            adapter.mcp_config_path(),
            tmp.path().join(".config/devin/mcp_config.json")
        );

        std::fs::create_dir_all(tmp.path().join(".codeium/windsurf")).unwrap();
        std::fs::write(
            tmp.path().join(".codeium/windsurf/mcp_config.json"),
            r#"{"mcpServers":{}}"#,
        )
        .unwrap();
        assert_eq!(
            adapter.mcp_config_path(),
            tmp.path().join(".codeium/windsurf/mcp_config.json")
        );

        std::fs::create_dir_all(tmp.path().join(".config/devin")).unwrap();
        std::fs::write(
            tmp.path().join(".config/devin/mcp_config.json"),
            r#"{"mcpServers":{}}"#,
        )
        .unwrap();
        assert_eq!(
            adapter.mcp_config_path(),
            tmp.path().join(".config/devin/mcp_config.json")
        );

        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join(".codeium")).unwrap();
        std::fs::write(
            tmp.path().join(".codeium/mcp_config.json"),
            r#"{"mcpServers":{}}"#,
        )
        .unwrap();
        assert_eq!(
            adapter.mcp_config_path(),
            tmp.path().join(".codeium/mcp_config.json")
        );
    }

    #[test]
    fn read_mcp_servers_reads_json_config() {
        let tmp = tempfile::tempdir().unwrap();
        let base_dir = tmp.path().join(".config/devin");
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::write(
            base_dir.join("mcp_config.json"),
            r#"{"mcpServers":{"github":{"command":"npx","args":["-y","server"],"env":{"TOKEN":"abc"}}}}"#,
        )
        .unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "github");
        assert_eq!(servers[0].command, "npx");
        assert_eq!(servers[0].args, vec!["-y", "server"]);
        assert_eq!(servers[0].env.get("TOKEN"), Some(&"abc".to_string()));
    }

    #[test]
    fn read_mcp_servers_merges_candidates_with_devin_priority() {
        let tmp = tempfile::tempdir().unwrap();
        let devin_dir = tmp.path().join(".config/devin");
        let legacy_dir = tmp.path().join(".codeium/windsurf");
        std::fs::create_dir_all(&devin_dir).unwrap();
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::write(
            devin_dir.join("mcp_config.json"),
            r#"{"mcpServers":{"shared":{"command":"new-cmd"}}}"#,
        )
        .unwrap();
        std::fs::write(
            legacy_dir.join("mcp_config.json"),
            r#"{"mcpServers":{"shared":{"command":"old-cmd"},"legacy-only":{"command":"old-only"}}}"#,
        )
        .unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 2);
        let shared = servers.iter().find(|s| s.name == "shared").unwrap();
        assert_eq!(shared.command, "new-cmd");
        assert!(servers.iter().any(|s| s.name == "legacy-only"));
    }

    #[test]
    fn read_hooks_reads_hooks_json() {
        let tmp = tempfile::tempdir().unwrap();
        let base_dir = tmp.path().join(".config/devin");
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::write(
            base_dir.join("hooks.json"),
            r#"{"hooks":{"pre_user_prompt":[{"command":"python3 /tmp/check.py"}],"post_cascade_response":[{"powershell":"python C:\\hooks\\log.py"}]}}"#,
        )
        .unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 2);
        assert!(hooks.iter().any(|hook| {
            hook.event == "pre_user_prompt" && hook.command == "python3 /tmp/check.py"
        }));
        assert!(hooks.iter().any(|hook| {
            hook.event == "post_cascade_response" && hook.command == "python C:\\hooks\\log.py"
        }));
    }

    #[test]
    fn read_hooks_merges_candidates_with_devin_priority() {
        let tmp = tempfile::tempdir().unwrap();
        let devin_dir = tmp.path().join(".config/devin");
        let legacy_dir = tmp.path().join(".codeium/windsurf");
        std::fs::create_dir_all(&devin_dir).unwrap();
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::write(
            devin_dir.join("hooks.json"),
            r#"{"hooks":{"pre_user_prompt":[{"command":"echo shared"}]}}"#,
        )
        .unwrap();
        std::fs::write(
            legacy_dir.join("hooks.json"),
            r#"{"hooks":{"pre_user_prompt":[{"command":"echo shared"},{"command":"echo legacy"}]}}"#,
        )
        .unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 2);
        assert!(hooks.iter().any(|hook| hook.command == "echo shared"));
        assert!(hooks.iter().any(|hook| hook.command == "echo legacy"));
    }

    #[test]
    fn global_memory_files_reads_markdown_files() {
        let tmp = tempfile::tempdir().unwrap();
        let memories_dir = tmp.path().join(".config/devin/memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(memories_dir.join("one.md"), "# One").unwrap();
        std::fs::write(memories_dir.join("two.txt"), "skip").unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let memories = adapter.global_memory_files();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].ends_with(".config/devin/memories/one.md"));
    }

    #[test]
    fn project_ignore_patterns_include_devin_and_legacy_ignores() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_ignore_patterns();
        assert!(patterns.contains(&".codeiumignore".to_string()));
        assert!(patterns.contains(&".windsurfignore".to_string()));
    }

    #[test]
    fn global_workflow_files_reads_markdown_files() {
        let tmp = tempfile::tempdir().unwrap();
        let workflows_dir = tmp.path().join(".config/devin/global_workflows");
        std::fs::create_dir_all(&workflows_dir).unwrap();
        std::fs::write(workflows_dir.join("deploy.md"), "# deploy").unwrap();
        std::fs::write(workflows_dir.join("notes.txt"), "skip").unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let files = adapter.global_workflow_files();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".config/devin/global_workflows/deploy.md"));
    }

    #[test]
    fn global_settings_files_excludes_workflows() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let files = adapter.global_settings_files();
        assert!(!files
            .iter()
            .any(|p| p.to_string_lossy().contains("global_workflows")));
    }

    #[test]
    fn project_workflow_patterns_includes_workflows_dir() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_workflow_patterns();
        assert_eq!(
            patterns,
            vec![
                ".devin/workflows/*.md".to_string(),
                ".windsurf/workflows/*.md".to_string()
            ]
        );
    }

    #[test]
    fn project_settings_patterns_excludes_workflows() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_settings_patterns();
        assert!(!patterns.iter().any(|p| p.contains("workflows")));
    }

    #[test]
    fn project_mcp_is_none_global_only() {
        // Windsurf MCP has no workspace config (official docs document only
        // `~/.codeium/windsurf/mcp_config.json`); pin the deliberate None so
        // it isn't "fixed" back by symmetry with other adapters.
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        assert!(adapter.project_mcp_config_relpath().is_none());
        assert!(!adapter
            .project_settings_patterns()
            .iter()
            .any(|p| p.contains("mcp_config")));
    }

    #[test]
    fn project_paths_prefer_devin_and_keep_windsurf_fallbacks() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        assert_eq!(
            adapter.project_skill_dirs(),
            vec![".devin/skills".to_string(), ".windsurf/skills".to_string()]
        );
        assert_eq!(
            adapter.project_hook_config_relpath().as_deref(),
            Some(".devin/hooks.json")
        );
        assert!(adapter
            .project_rules_patterns()
            .contains(&".devin/rules/**/*.md".to_string()));
        assert!(adapter
            .project_rules_patterns()
            .contains(&".windsurfrules".to_string()));
        assert!(adapter
            .project_rules_patterns()
            .contains(&".windsurf/rules/**/*.md".to_string()));
    }

    #[test]
    fn project_hook_paths_scan_devin_and_legacy() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let scope = ConfigScope::Project {
            name: "demo".into(),
            path: tmp.path().join("project").to_string_lossy().to_string(),
        };
        assert_eq!(
            adapter.hook_config_paths_for(&scope),
            vec![
                tmp.path().join("project/.devin/hooks.json"),
                tmp.path().join("project/.windsurf/hooks.json")
            ]
        );
    }
}
