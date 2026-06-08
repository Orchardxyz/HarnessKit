import { create } from "zustand";

export type ThemeName = "tiesen" | "claude";
export type Mode = "system" | "dark" | "light";
export type AppIcon = "icon-1" | "icon-2";
export type AgentVisibility = "all" | "detected";

/**
 * Safely retrieves and validates a localStorage value against allowed values.
 * Falls back to the default if localStorage is unavailable or the value is invalid.
 */
function getValidItem<T extends string>(
  key: string,
  allowed: readonly T[],
  fallback: T,
): T {
  if (typeof localStorage === "undefined") return fallback;
  const val = localStorage.getItem(key);
  return val && (allowed as readonly string[]).includes(val)
    ? (val as T)
    : fallback;
}

/**
 * Reads a JSON string array from localStorage, falling back to an empty array
 * if storage is unavailable, missing, or malformed.
 */
function getStoredStringArray(key: string): string[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) && parsed.every((v) => typeof v === "string")
      ? parsed
      : [];
  } catch {
    return [];
  }
}

interface UIState {
  sidebarOpen: boolean;
  themeName: ThemeName;
  mode: Mode;
  appIcon: AppIcon;
  agentVisibility: AgentVisibility;
  /**
   * Agents that "Detected only" visibility auto-disabled, so switching back to
   * "All agents" can re-enable exactly those (and not agents the user disabled
   * manually). Persisted so the restore survives a restart.
   */
  autoDisabledAgents: string[];
  toggleSidebar: () => void;
  setThemeName: (name: ThemeName) => void;
  setMode: (mode: Mode) => void;
  setAppIcon: (icon: AppIcon) => void;
  setAgentVisibility: (visibility: AgentVisibility) => void;
  setAutoDisabledAgents: (names: string[]) => void;
}

const ALLOWED_MODES: readonly Mode[] = ["system", "dark", "light"];
const ALLOWED_THEME_NAMES: readonly ThemeName[] = ["tiesen", "claude"];
const ALLOWED_APP_ICONS: readonly AppIcon[] = ["icon-1", "icon-2"];
const ALLOWED_AGENT_VISIBILITY: readonly AgentVisibility[] = [
  "all",
  "detected",
];

const storedMode = getValidItem("hk-theme", ALLOWED_MODES, "system");
const storedThemeName = getValidItem(
  "hk-theme-name",
  ALLOWED_THEME_NAMES,
  "tiesen",
);
const storedAppIcon = getValidItem("hk-app-icon", ALLOWED_APP_ICONS, "icon-1");
const storedAgentVisibility = getValidItem(
  "hk-agent-visibility",
  ALLOWED_AGENT_VISIBILITY,
  "all",
);
const storedAutoDisabledAgents = getStoredStringArray("hk-agent-auto-disabled");

/** Resolve "system" to actual light/dark based on OS preference */
export function resolveMode(mode: Mode): "dark" | "light" {
  if (mode !== "system") return mode;
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  themeName: storedThemeName,
  mode: storedMode,
  appIcon: storedAppIcon,
  agentVisibility: storedAgentVisibility,
  autoDisabledAgents: storedAutoDisabledAgents,
  toggleSidebar() {
    set((s) => ({ sidebarOpen: !s.sidebarOpen }));
  },
  setThemeName(themeName) {
    localStorage.setItem("hk-theme-name", themeName);
    set({ themeName });
  },
  setMode(mode) {
    localStorage.setItem("hk-theme", mode);
    set({ mode });
  },
  setAppIcon(appIcon) {
    localStorage.setItem("hk-app-icon", appIcon);
    set({ appIcon });
  },
  setAgentVisibility(agentVisibility) {
    localStorage.setItem("hk-agent-visibility", agentVisibility);
    set({ agentVisibility });
  },
  setAutoDisabledAgents(autoDisabledAgents) {
    localStorage.setItem(
      "hk-agent-auto-disabled",
      JSON.stringify(autoDisabledAgents),
    );
    set({ autoDisabledAgents });
  },
}));
