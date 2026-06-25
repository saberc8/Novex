export type ResearchLocale = "zh-CN" | "en-US";

export type ResearchLocaleStorage = {
  getItem: (key: string) => string | null;
  setItem: (key: string, value: string) => void;
};

export const DEFAULT_RESEARCH_LOCALE: ResearchLocale = "zh-CN";
export const RESEARCH_LOCALE_STORAGE_KEY = "novex.researchRadar.locale";

export const RESEARCH_LOCALE_OPTIONS: Array<{ locale: ResearchLocale; label: string }> = [
  { locale: "zh-CN", label: "中文" },
  { locale: "en-US", label: "English" }
];

export function normalizeResearchLocale(value: unknown): ResearchLocale {
  return value === "en-US" || value === "zh-CN" ? value : DEFAULT_RESEARCH_LOCALE;
}

export function readSavedResearchLocale(storage: ResearchLocaleStorage | null = browserStorage()): ResearchLocale {
  if (!storage) {
    return DEFAULT_RESEARCH_LOCALE;
  }

  try {
    return normalizeResearchLocale(storage.getItem(RESEARCH_LOCALE_STORAGE_KEY));
  } catch {
    return DEFAULT_RESEARCH_LOCALE;
  }
}

export function saveResearchLocale(
  locale: ResearchLocale,
  storage: ResearchLocaleStorage | null = browserStorage()
) {
  if (!storage) {
    return;
  }

  try {
    storage.setItem(RESEARCH_LOCALE_STORAGE_KEY, locale);
  } catch {
    // Keep the in-memory locale even when persistence is blocked.
  }
}

export function researchReportLanguageInstruction(locale: ResearchLocale) {
  if (locale === "en-US") {
    return "Write the markdown report in English. Keep source titles, project names, dataset names, URLs, and author names in their original language.";
  }

  return "请用中文撰写 markdown 报告。论文标题、项目名、数据集名、URL 和作者名保留原始语言";
}

function browserStorage(): ResearchLocaleStorage | null {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    return window.localStorage;
  } catch {
    return null;
  }
}
