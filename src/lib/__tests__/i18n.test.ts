import { beforeEach, describe, expect, it, vi } from "vitest";

function setNavigatorLanguage(
  language: string,
  languages: string[] = [language],
): void {
  Object.defineProperty(window.navigator, "language", {
    configurable: true,
    value: language,
  });
  Object.defineProperty(window.navigator, "languages", {
    configurable: true,
    value: languages,
  });
}

describe("i18n language preference helpers", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
    setNavigatorLanguage("en-US");
  });

  it("defaults to system preference when storage is empty", async () => {
    const { getStoredLanguagePreference } = await import("../i18n");
    expect(getStoredLanguagePreference()).toBe("system");
  });

  it("keeps explicit language preferences from storage", async () => {
    localStorage.setItem("hk-language", "zh");
    const { getStoredLanguagePreference } = await import("../i18n");
    expect(getStoredLanguagePreference()).toBe("zh");
  });

  it("treats invalid stored values as system", async () => {
    localStorage.setItem("hk-language", "ja");
    const { getStoredLanguagePreference } = await import("../i18n");
    expect(getStoredLanguagePreference()).toBe("system");
  });

  it("maps system locales to supported languages", async () => {
    const { mapLocaleToSupportedLanguage } = await import("../i18n");
    expect(mapLocaleToSupportedLanguage("zh-CN")).toBe("zh");
    expect(mapLocaleToSupportedLanguage("en-GB")).toBe("en");
    expect(mapLocaleToSupportedLanguage("ja-JP")).toBeNull();
  });

  it("resolves system preference from navigator languages", async () => {
    setNavigatorLanguage("fr-FR", ["fr-FR", "zh-CN"]);
    const { resolveLanguagePreference } = await import("../i18n");
    expect(resolveLanguagePreference("system")).toBe("zh");
  });

  it("falls back to English for unsupported system locales", async () => {
    setNavigatorLanguage("ja-JP");
    const { resolveLanguagePreference } = await import("../i18n");
    expect(resolveLanguagePreference("system")).toBe("en");
  });

  it("applies system preference without overwriting the stored setting", async () => {
    setNavigatorLanguage("zh-CN");
    const { applyLanguagePreference, default: i18n } = await import("../i18n");

    await applyLanguagePreference("system");

    expect(localStorage.getItem("hk-language")).toBe("system");
    expect(i18n.resolvedLanguage).toBe("zh");
  });

  it("applies explicit preferences directly", async () => {
    setNavigatorLanguage("en-US");
    const { applyLanguagePreference, default: i18n } = await import("../i18n");

    await applyLanguagePreference("zh");

    expect(localStorage.getItem("hk-language")).toBe("zh");
    expect(i18n.resolvedLanguage).toBe("zh");
  });
});
