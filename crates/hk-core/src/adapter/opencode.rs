use super::{AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, PluginEntry};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct OpencodeAdapter {
    home: PathBuf,
}

impl Default for OpencodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpencodeAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    fn parse_json(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn files_with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return vec![];
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == ext))
            .collect()
    }

    fn plugin_name(path: &Path) -> String {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let base = file_name.strip_suffix(".disabled").unwrap_or(&file_name);
        Path::new(base)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| base.to_string())
    }

    fn is_plugin_file(path: &Path) -> bool {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let base = file_name.strip_suffix(".disabled").unwrap_or(&file_name);
        matches!(
            Path::new(base).extension().and_then(|ext| ext.to_str()),
            Some("js" | "ts" | "mjs" | "cjs")
        )
    }

    fn parse_local_mcp_entry(name: &str, value: &serde_json::Value) -> Option<McpServerEntry> {
        if value.get("type").and_then(|v| v.as_str()) != Some("local") {
            return None;
        }
        // Honor OpenCode's `enabled: false` — when the user explicitly disabled
        // the server in opencode.json, HarnessKit hides it from its view (same
        // as the remote-type filter above). Field default is true, so omitted
        // or `enabled: true` flows through normally.
        // Spec: https://opencode.ai/docs/mcp-servers/ ("Enable or disable the
        // MCP server on startup").
        if value.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
            return None;
        }

        let (command, args) = match value.get("command") {
            Some(serde_json::Value::Array(parts)) => {
                let mut parts = parts
                    .iter()
                    .filter_map(|part| part.as_str().map(String::from));
                let command = parts.next()?;
                (command, parts.collect())
            }
            Some(serde_json::Value::String(command)) => (command.clone(), vec![]),
            _ => return None,
        };

        let env = value
            .get("environment")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        Some(McpServerEntry {
            name: name.to_string(),
            command,
            args,
            env,
        })
    }
}

impl AgentAdapter for OpencodeAdapter {
    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::Opencode
    }

    fn name(&self) -> &str {
        "opencode"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".config").join("opencode")
    }

    fn detect(&self) -> bool {
        // Aligned with the other 7 adapters (claude/codex/gemini/cursor/
        // antigravity/copilot/windsurf) which all detect by base_dir presence
        // alone. A `which opencode` hit without a config dir would surface an
        // agent that has nothing for HarnessKit to manage — a UX false
        // positive — so we keep this strict.
        self.base_dir().exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }

    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("opencode.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.mcp_config_path()
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("plugins")]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(config) = Self::parse_json(path) else {
            return vec![];
        };
        let Some(servers) = config.get("mcp").and_then(|v| v.as_object()) else {
            return vec![];
        };
        servers
            .iter()
            .filter_map(|(name, value)| Self::parse_local_mcp_entry(name, value))
            .collect()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        let mut entries = Vec::new();
        for plugin_dir in self.plugin_dirs() {
            let Ok(files) = std::fs::read_dir(plugin_dir) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                if !path.is_file() || !Self::is_plugin_file(&path) {
                    continue;
                }
                let enabled = path.extension().is_none_or(|ext| ext != "disabled");
                entries.push(PluginEntry {
                    name: Self::plugin_name(&path),
                    source: "local".into(),
                    enabled,
                    path: Some(path),
                    uri: None,
                    installed_at: None,
                    updated_at: None,
                });
            }
        }
        entries
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("AGENTS.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        // Includes the canonical config file plus the .jsonc variant (only one
        // exists per install, but listing both lets the scanner find either),
        // and every directory whose contents are user-configurable settings:
        //   - agents/*.md  : agent definitions
        //   - modes/*.md   : agent mode definitions
        //   - themes/*.json: UI themes (palette/styling JSON)
        // tools/*.ts and plugins/*.ts are intentionally excluded — they are
        // code, not settings, and have their own discovery paths.
        let base = self.base_dir();
        let mut files = vec![
            self.mcp_config_path(),       // opencode.json
            base.join("opencode.jsonc"),  // jsonc variant
        ];
        files.extend(Self::files_with_ext(&base.join("agents"), "md"));
        files.extend(Self::files_with_ext(&base.join("modes"), "md"));
        files.extend(Self::files_with_ext(&base.join("themes"), "json"));
        files
    }

    fn global_workflow_files(&self) -> Vec<PathBuf> {
        Self::files_with_ext(&self.base_dir().join("commands"), "md")
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        // Rules: project-root AGENTS.md (https://opencode.ai/docs/rules/).
        // OpenCode also falls back to CLAUDE.md when AGENTS.md is absent, but
        // we don't claim CLAUDE.md here — that file's primary owner is Claude
        // Code, and surfacing it as an OpenCode rule would be misleading.
        vec!["AGENTS.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        // Project config: <project_root>/opencode.json[c]
        // (https://opencode.ai/docs/config/). Both .json and .jsonc are valid.
        vec!["opencode.json".into(), "opencode.jsonc".into()]
    }

    fn project_workflow_patterns(&self) -> Vec<String> {
        // Slash commands: .opencode/commands/*.md
        // (https://opencode.ai/docs/commands/).
        vec![".opencode/commands/*.md".into()]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".opencode/skills".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        // OpenCode reads MCP servers from the same opencode.json that holds
        // every other setting; there is no separate `.mcp.json`. The single
        // canonical name is preferred — users on .jsonc miss project-level
        // MCP discovery, but that matches how every other adapter (Claude
        // `.mcp.json`, Cursor `.cursor/mcp.json`) picks one canonical path.
        Some("opencode.json".into())
    }

    fn project_plugin_dirs(&self) -> Vec<String> {
        // Project plugins: .opencode/plugins/ holds JS/TS files
        // (https://opencode.ai/docs/plugins/). Same loader as global plugins.
        vec![".opencode/plugins".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn detect_requires_base_dir() {
        // Empty home → not detected: prevents surfacing OpenCode in the agents
        // list when the user has only the CLI installed but no config dir,
        // matching how every other adapter checks detection.
        let tmp = tempfile::tempdir().unwrap();
        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());

        // base_dir present (with or without contents) → detected.
        std::fs::create_dir_all(tmp.path().join(".config/opencode")).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn read_mcp_servers_keeps_only_local_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".config/opencode");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("opencode.json"),
            r#"{
                "mcp": {
                    "local-server": {
                        "type": "local",
                        "command": ["bun", "x", "tool"],
                        "environment": {"TOKEN": "abc"}
                    },
                    "remote-server": {
                        "type": "remote",
                        "url": "https://example.com/mcp"
                    }
                }
            }"#,
        )
        .unwrap();

        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "local-server");
        assert_eq!(servers[0].command, "bun");
        assert_eq!(servers[0].args, vec!["x", "tool"]);
        assert_eq!(servers[0].env.get("TOKEN"), Some(&"abc".to_string()));
    }

    #[test]
    fn read_mcp_servers_skips_explicitly_disabled() {
        // OpenCode lets users keep a server in opencode.json but switch it off
        // via `enabled: false`. HarnessKit treats those as if they don't exist
        // — surfacing them as "active" would mislead the UI and any toggle
        // operation against them would silently fight the user's config.
        // Default-true (omitted or `enabled: true`) must still flow through.
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".config/opencode");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("opencode.json"),
            r#"{
                "mcp": {
                    "active": {
                        "type": "local",
                        "command": ["bin"]
                    },
                    "explicit-on": {
                        "type": "local",
                        "command": ["bin"],
                        "enabled": true
                    },
                    "explicit-off": {
                        "type": "local",
                        "command": ["bin"],
                        "enabled": false
                    }
                }
            }"#,
        )
        .unwrap();

        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        let names: Vec<_> = adapter
            .read_mcp_servers()
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(names.contains(&"active".to_string()));
        assert!(names.contains(&"explicit-on".to_string()));
        assert!(
            !names.contains(&"explicit-off".to_string()),
            "enabled:false entries must be filtered out"
        );
    }

    #[test]
    fn read_plugins_scans_enabled_and_disabled_files() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins_dir = tmp.path().join(".config/opencode/plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("alpha.ts"), "export default {};").unwrap();
        std::fs::write(
            plugins_dir.join("beta.js.disabled"),
            "module.exports = {};",
        )
        .unwrap();

        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 2);
        assert!(plugins.iter().any(|plugin| {
            plugin.name == "alpha" && plugin.enabled && plugin.source == "local"
        }));
        assert!(plugins.iter().any(|plugin| {
            plugin.name == "beta" && !plugin.enabled && plugin.source == "local"
        }));
    }

    #[test]
    fn global_settings_and_workflows_include_configurable_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join(".config/opencode");
        for sub in ["agents", "modes", "themes", "commands", "tools"] {
            std::fs::create_dir_all(base.join(sub)).unwrap();
        }
        std::fs::write(base.join("agents/reviewer.md"), "# reviewer").unwrap();
        std::fs::write(base.join("modes/build.md"), "# build mode").unwrap();
        std::fs::write(base.join("themes/dark.json"), "{}").unwrap();
        std::fs::write(base.join("commands/deploy.md"), "# deploy").unwrap();
        // tools/ is code — must NOT appear in settings.
        std::fs::write(base.join("tools/lint.ts"), "export default {}").unwrap();
        // Non-matching extensions inside scanned dirs must be ignored.
        std::fs::write(base.join("agents/notes.txt"), "ignore me").unwrap();

        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        let settings = adapter.global_settings_files();
        let workflows = adapter.global_workflow_files();

        // Three subdir kinds plus the two top-level config paths.
        assert!(settings.iter().any(|p| p.ends_with("agents/reviewer.md")));
        assert!(settings.iter().any(|p| p.ends_with("modes/build.md")));
        assert!(settings.iter().any(|p| p.ends_with("themes/dark.json")));
        assert!(settings.iter().any(|p| p.ends_with("opencode.json")));
        assert!(settings.iter().any(|p| p.ends_with("opencode.jsonc")));

        // Code-bearing dirs and non-matching extensions are excluded.
        assert!(
            !settings.iter().any(|p| p.ends_with("tools/lint.ts")),
            "tools/ holds code, must not be in settings"
        );
        assert!(
            !settings.iter().any(|p| p.ends_with("notes.txt")),
            "files with non-md extensions in agents/ must be filtered"
        );

        // Workflows still flows through commands/.
        assert!(workflows.iter().any(|p| p.ends_with("commands/deploy.md")));
    }

    #[test]
    fn project_level_config_paths_match_upstream_conventions() {
        // Pin the exact relative paths/patterns the project scanner uses to
        // discover OpenCode config inside a user's repository. Cross-checked
        // against opencode.ai/docs (config, rules, commands, plugins, mcp-servers).
        let adapter = OpencodeAdapter::new();

        // Rules: project-root AGENTS.md (no .override.md variant exists).
        assert_eq!(adapter.project_rules_patterns(), vec!["AGENTS.md".to_string()]);

        // Settings file lives at project root, both .json and .jsonc accepted.
        assert_eq!(
            adapter.project_settings_patterns(),
            vec!["opencode.json".to_string(), "opencode.jsonc".to_string()]
        );

        // Slash commands.
        assert_eq!(
            adapter.project_workflow_patterns(),
            vec![".opencode/commands/*.md".to_string()]
        );

        // MCP project config sits inside the same opencode.json, not a
        // separate .mcp.json — distinct from Claude's split-file convention.
        assert_eq!(
            adapter.project_mcp_config_relpath(),
            Some("opencode.json".to_string())
        );

        // Project plugins.
        assert_eq!(
            adapter.project_plugin_dirs(),
            vec![".opencode/plugins".to_string()]
        );

        // Hooks: OpenCode hooks are JS/TS plugin code, not JSON config, so
        // there is no project hook config file to discover. Stays None.
        assert_eq!(adapter.project_hook_config_relpath(), None);
    }
}
