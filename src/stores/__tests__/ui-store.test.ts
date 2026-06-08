import { beforeEach, describe, expect, it, vi } from "vitest";

describe("ui-store localStorage validation", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
  });

  it("uses defaults when localStorage is empty", async () => {
    const { useUIStore } = await import("../ui-store");
    const state = useUIStore.getState();
    expect(state.mode).toBe("system");
    expect(state.themeName).toBe("tiesen");
    expect(state.appIcon).toBe("icon-1");
    expect(state.sidebarOpen).toBe(true);
    expect(state.agentVisibility).toBe("all");
    expect(state.autoDisabledAgents).toEqual([]);
  });

  it("reads valid localStorage values", async () => {
    localStorage.setItem("hk-theme", "dark");
    localStorage.setItem("hk-theme-name", "claude");
    localStorage.setItem("hk-app-icon", "icon-2");
    localStorage.setItem("hk-agent-visibility", "detected");
    localStorage.setItem(
      "hk-agent-auto-disabled",
      JSON.stringify(["cursor", "gemini"]),
    );

    const { useUIStore } = await import("../ui-store");
    const state = useUIStore.getState();
    expect(state.mode).toBe("dark");
    expect(state.themeName).toBe("claude");
    expect(state.appIcon).toBe("icon-2");
    expect(state.agentVisibility).toBe("detected");
    expect(state.autoDisabledAgents).toEqual(["cursor", "gemini"]);
  });

  it("ignores invalid localStorage values and falls back to defaults", async () => {
    localStorage.setItem("hk-theme", "INVALID_MODE");
    localStorage.setItem("hk-theme-name", "INVALID_THEME");
    localStorage.setItem("hk-app-icon", "INVALID_ICON");
    localStorage.setItem("hk-agent-visibility", "INVALID_VISIBILITY");
    localStorage.setItem("hk-agent-auto-disabled", "not-json{[");

    const { useUIStore } = await import("../ui-store");
    const state = useUIStore.getState();
    expect(state.mode).toBe("system");
    expect(state.themeName).toBe("tiesen");
    expect(state.appIcon).toBe("icon-1");
    expect(state.agentVisibility).toBe("all");
    expect(state.autoDisabledAgents).toEqual([]);
  });

  it("ignores a non-string-array auto-disabled value", async () => {
    localStorage.setItem(
      "hk-agent-auto-disabled",
      JSON.stringify([1, "ok", true]),
    );

    const { useUIStore } = await import("../ui-store");
    expect(useUIStore.getState().autoDisabledAgents).toEqual([]);
  });

  it("setMode persists to localStorage", async () => {
    const { useUIStore } = await import("../ui-store");
    useUIStore.getState().setMode("dark");
    expect(localStorage.getItem("hk-theme")).toBe("dark");
    expect(useUIStore.getState().mode).toBe("dark");
  });

  it("setThemeName persists to localStorage", async () => {
    const { useUIStore } = await import("../ui-store");
    useUIStore.getState().setThemeName("claude");
    expect(localStorage.getItem("hk-theme-name")).toBe("claude");
    expect(useUIStore.getState().themeName).toBe("claude");
  });

  it("toggleSidebar flips the boolean", async () => {
    const { useUIStore } = await import("../ui-store");
    expect(useUIStore.getState().sidebarOpen).toBe(true);
    useUIStore.getState().toggleSidebar();
    expect(useUIStore.getState().sidebarOpen).toBe(false);
    useUIStore.getState().toggleSidebar();
    expect(useUIStore.getState().sidebarOpen).toBe(true);
  });

  it("setAgentVisibility persists to localStorage", async () => {
    const { useUIStore } = await import("../ui-store");
    useUIStore.getState().setAgentVisibility("detected");
    expect(localStorage.getItem("hk-agent-visibility")).toBe("detected");
    expect(useUIStore.getState().agentVisibility).toBe("detected");
  });

  it("setAutoDisabledAgents persists the snapshot as JSON", async () => {
    const { useUIStore } = await import("../ui-store");
    useUIStore.getState().setAutoDisabledAgents(["cursor", "gemini"]);
    expect(localStorage.getItem("hk-agent-auto-disabled")).toBe(
      JSON.stringify(["cursor", "gemini"]),
    );
    expect(useUIStore.getState().autoDisabledAgents).toEqual([
      "cursor",
      "gemini",
    ]);

    useUIStore.getState().setAutoDisabledAgents([]);
    expect(localStorage.getItem("hk-agent-auto-disabled")).toBe("[]");
    expect(useUIStore.getState().autoDisabledAgents).toEqual([]);
  });
});
