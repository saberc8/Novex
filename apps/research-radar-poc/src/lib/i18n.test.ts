import { describe, expect, it, vi } from "vitest";
import {
  DEFAULT_RESEARCH_LOCALE,
  RESEARCH_LOCALE_STORAGE_KEY,
  normalizeResearchLocale,
  readSavedResearchLocale,
  researchReportLanguageInstruction,
  saveResearchLocale
} from "./i18n";

function memoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: vi.fn((key: string) => values.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => {
      values.set(key, value);
    })
  };
}

describe("research radar i18n", () => {
  it("defaults unsupported locale values to Chinese", () => {
    expect(DEFAULT_RESEARCH_LOCALE).toBe("zh-CN");
    expect(normalizeResearchLocale("zh-CN")).toBe("zh-CN");
    expect(normalizeResearchLocale("en-US")).toBe("en-US");
    expect(normalizeResearchLocale("fr-FR")).toBe("zh-CN");
    expect(normalizeResearchLocale(null)).toBe("zh-CN");
  });

  it("reads and saves a valid locale from storage", () => {
    const storage = memoryStorage({ [RESEARCH_LOCALE_STORAGE_KEY]: "en-US" });
    expect(readSavedResearchLocale(storage)).toBe("en-US");

    saveResearchLocale("zh-CN", storage);
    expect(storage.setItem).toHaveBeenCalledWith(RESEARCH_LOCALE_STORAGE_KEY, "zh-CN");
  });

  it("falls back to Chinese when storage has invalid values or throws", () => {
    expect(readSavedResearchLocale(memoryStorage({ [RESEARCH_LOCALE_STORAGE_KEY]: "bad" }))).toBe("zh-CN");
    expect(readSavedResearchLocale({
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {}
    })).toBe("zh-CN");
  });

  it("builds report language instructions for both supported locales", () => {
    expect(researchReportLanguageInstruction("zh-CN")).toContain("中文");
    expect(researchReportLanguageInstruction("en-US")).toContain("English");
  });
});
