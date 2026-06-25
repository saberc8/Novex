# Research Radar i18n Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Chinese-by-default language support to the Research Radar POC with an English toggle and localized Agent report instructions.

**Architecture:** Add a small typed frontend i18n layer instead of introducing a dependency. The app owns locale state, persists it in local storage, passes it into Agent run creation, and supplies localized copy to UI components. API source scanning and third-party source content remain unchanged.

**Tech Stack:** Next.js 16, React 19, TypeScript, Tailwind CSS, Vitest, Testing Library, localStorage.

## Global Constraints

- Default language is `zh-CN`.
- Supported languages are `zh-CN` and `en-US`.
- If the user has selected a language before, load it from local storage.
- If no saved choice exists, use `zh-CN`. Do not auto-switch based on browser language in this pass.
- The selected language affects UI copy and the Agent report language instruction.
- Source results, paper titles, project names, URLs, author names, and dataset names remain in their original source language.
- Full app-wide internationalization outside `apps/research-radar-poc` is out of scope.
- Backend locale negotiation is out of scope.
- Runtime translation of existing model output after it has already been generated is out of scope.
- Existing user change `apps/codex-app-poc/app/layout.tsx` must not be modified, staged, or reverted.

---

## File Structure

- Create `apps/research-radar-poc/src/lib/i18n.ts`
  - Owns `ResearchLocale`, locale labels, local storage helpers, report-language prompt text, and the typed UI dictionary.
- Create `apps/research-radar-poc/src/lib/i18n.test.ts`
  - Unit tests for locale normalization, default behavior, invalid saved values, saving, and language prompt text.
- Modify `apps/research-radar-poc/src/types/research.ts`
  - Adds optional `locale?: ResearchLocale` to `ResearchScanInput`.
- Modify `apps/research-radar-poc/src/api/research.ts`
  - Adds selected-language instruction to Agent prompt while preserving the 4000-character cap.
- Modify `apps/research-radar-poc/src/api/research.test.ts`
  - Tests Chinese and English prompt instructions and keeps cap coverage.
- Modify `apps/research-radar-poc/src/app-client.tsx`
  - Owns locale state, renders the language selector, localizes UI copy, and passes locale into `createResearchRadarRun`.
- Modify `apps/research-radar-poc/src/components/research-map.tsx`
  - Accepts localized copy for map labels, layers, empty state, caveats, and node kind labels.
- Modify `apps/research-radar-poc/src/components/research-map.test.tsx`
  - Tests default English fallback for standalone component and Chinese copy supplied by app.
- Modify `apps/research-radar-poc/app/page.test.tsx`
  - Tests default Chinese UI, English switching, saved locale restore, invalid locale fallback, and locale in the run request.

### Task 1: Locale Primitives and Agent Prompt Language

**Files:**
- Create: `apps/research-radar-poc/src/lib/i18n.ts`
- Create: `apps/research-radar-poc/src/lib/i18n.test.ts`
- Modify: `apps/research-radar-poc/src/types/research.ts`
- Modify: `apps/research-radar-poc/src/api/research.ts`
- Modify: `apps/research-radar-poc/src/api/research.test.ts`

**Interfaces:**
- Produces:
  - `type ResearchLocale = "zh-CN" | "en-US"`
  - `const DEFAULT_RESEARCH_LOCALE: ResearchLocale`
  - `const RESEARCH_LOCALE_STORAGE_KEY: string`
  - `const RESEARCH_LOCALE_OPTIONS: Array<{ locale: ResearchLocale; label: string }>`
  - `function normalizeResearchLocale(value: unknown): ResearchLocale`
  - `function readSavedResearchLocale(storage?: ResearchLocaleStorage | null): ResearchLocale`
  - `function saveResearchLocale(locale: ResearchLocale, storage?: ResearchLocaleStorage | null): void`
  - `function researchReportLanguageInstruction(locale: ResearchLocale): string`
- Consumes:
  - `ResearchScanInput` in `src/api/research.ts` gains `locale?: ResearchLocale`.

- [ ] **Step 1: Write failing i18n utility tests**

Add `apps/research-radar-poc/src/lib/i18n.test.ts`:

```ts
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
```

- [ ] **Step 2: Run i18n tests to verify RED**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/lib/i18n.test.ts
```

Expected: fail because `src/lib/i18n.ts` does not exist.

- [ ] **Step 3: Implement locale primitives**

Create `apps/research-radar-poc/src/lib/i18n.ts`:

```ts
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

  return "请用中文撰写 markdown 报告。论文标题、项目名、数据集名、URL 和作者名保留原始语言。";
}

function browserStorage(): ResearchLocaleStorage | null {
  if (typeof window === "undefined") {
    return null;
  }
  return window.localStorage;
}
```

- [ ] **Step 4: Run i18n tests to verify GREEN**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/lib/i18n.test.ts
```

Expected: pass, 4 tests.

- [ ] **Step 5: Write failing API prompt tests**

In `apps/research-radar-poc/src/api/research.test.ts`, add:

```ts
  it("asks for a Chinese markdown report by default", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "agent workflow",
      filters: ["papers"],
      ranking: "balanced",
      routeId: "runtime.llm"
    });

    expect(command.input).toContain("请用中文撰写 markdown 报告");
  });

  it("asks for an English markdown report when English is selected", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "agent workflow",
      filters: ["papers"],
      ranking: "balanced",
      routeId: "runtime.llm",
      locale: "en-US"
    });

    expect(command.input).toContain("Write the markdown report in English");
  });
```

- [ ] **Step 6: Run API tests to verify RED**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/api/research.test.ts
```

Expected: fail because the prompt does not include locale-specific language instructions and `ResearchScanInput` has no `locale` property.

- [ ] **Step 7: Extend research types and prompt builder**

Modify `apps/research-radar-poc/src/types/research.ts`:

```ts
import type { ResearchLocale } from "@/lib/i18n";
```

Add to `ResearchScanInput`:

```ts
  locale?: ResearchLocale;
```

Modify `apps/research-radar-poc/src/api/research.ts`:

```ts
import { normalizeResearchLocale, researchReportLanguageInstruction } from "@/lib/i18n";
```

Inside `buildResearchRadarPrompt` before `beforeEvidence`:

```ts
  const locale = normalizeResearchLocale(input.locale);
```

Add this line to `afterEvidence` before `"Return a concise markdown report with exactly these headings:"`:

```ts
    researchReportLanguageInstruction(locale),
```

- [ ] **Step 8: Run API tests and cap coverage**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/api/research.test.ts
```

Expected: pass, including the existing long evidence cap test.

- [ ] **Step 9: Commit Task 1**

```bash
git add apps/research-radar-poc/src/lib/i18n.ts \
  apps/research-radar-poc/src/lib/i18n.test.ts \
  apps/research-radar-poc/src/types/research.ts \
  apps/research-radar-poc/src/api/research.ts \
  apps/research-radar-poc/src/api/research.test.ts
git commit -m "feat: add research radar locale prompt support"
```

### Task 2: App Locale State and Language Selector

**Files:**
- Modify: `apps/research-radar-poc/src/lib/i18n.ts`
- Modify: `apps/research-radar-poc/src/app-client.tsx`
- Modify: `apps/research-radar-poc/app/page.test.tsx`

**Interfaces:**
- Consumes from Task 1:
  - `ResearchLocale`
  - `RESEARCH_LOCALE_OPTIONS`
  - `readSavedResearchLocale`
  - `saveResearchLocale`
- Produces:
  - `researchRadarCopy(locale)` dictionary accessor.
  - `LanguageSelector` rendered in the header.
  - `locale` passed into `createResearchRadarRun`.

- [ ] **Step 1: Write failing page tests for default Chinese and English switching**

In `apps/research-radar-poc/app/page.test.tsx`, update the first test and add two tests:

```ts
  it("renders the workbench in Chinese by default", () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        json: async () => ({ code: "200", data: { list: [], total: 0 } })
      }))
    );

    render(<Page />);

    expect(screen.getByRole("heading", { name: "Research Radar" })).toBeTruthy();
    expect(screen.getByLabelText("研究主题")).toBeTruthy();
    expect(screen.getByText("论文")).toBeTruthy();
    expect(screen.getByText("开源项目")).toBeTruthy();
    expect(screen.getByText("数据集")).toBeTruthy();
    expect(screen.getByText("基准")).toBeTruthy();
    expect(screen.getByText("新闻")).toBeTruthy();
    expect(screen.getByText("社区")).toBeTruthy();
    expect(screen.getByRole("button", { name: "均衡" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "启动雷达扫描" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "中文" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "English" })).toBeTruthy();
  });

  it("switches visible workbench copy to English", () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, json: async () => ({ code: "200", data: {} }) })));

    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "English" }));

    expect(screen.getByLabelText("Research topic")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Balanced" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start radar scan" })).toBeTruthy();
    expect(window.localStorage.getItem("novex.researchRadar.locale")).toBe("en-US");
  });

  it("restores saved English locale and ignores invalid saved locale", () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, json: async () => ({ code: "200", data: {} }) })));
    window.localStorage.setItem("novex.researchRadar.locale", "en-US");

    const { unmount } = render(<Page />);
    expect(screen.getByLabelText("Research topic")).toBeTruthy();
    unmount();

    window.localStorage.setItem("novex.researchRadar.locale", "bad");
    render(<Page />);
    expect(screen.getByLabelText("研究主题")).toBeTruthy();
  });
```

- [ ] **Step 2: Run page tests to verify RED**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx
```

Expected: fail because the app still has English filter/ranking labels, no language selector, and no saved locale handling.

- [ ] **Step 3: Add typed UI copy to i18n**

In `apps/research-radar-poc/src/lib/i18n.ts`, add:

```ts
export type ResearchRadarCopy = {
  status: {
    ready: string;
    scanning: string;
    failed: string;
    pending: string;
    running: string;
    succeeded: string;
    limited: string;
  };
  sidebar: {
    title: string;
    empty: string;
  };
  composer: {
    label: string;
    placeholder: string;
    submit: string;
    emptyError: string;
    sourceScanFailed: string;
    modelUnavailable: string;
    filters: Record<"papers" | "projects" | "datasets" | "benchmarks" | "news" | "community", string>;
    rankings: Record<"balanced" | "importance" | "recency" | "beginner", string>;
  };
  preview: {
    signal: string;
    signalValue: string;
    people: string;
    peopleValue: string;
    experiments: string;
    experimentsValue: string;
  };
  workspace: {
    sources: string;
    events: string;
    sections: string;
  };
  evidence: {
    title: string;
    waiting: string;
    liveModelOutput: string;
    chunks: string;
    run: string;
    status: string;
    trace: string;
  };
};

export const RESEARCH_RADAR_COPY: Record<ResearchLocale, ResearchRadarCopy> = {
  "zh-CN": {
    status: {
      ready: "就绪",
      scanning: "扫描中",
      failed: "失败",
      pending: "等待中",
      running: "运行中",
      succeeded: "成功",
      limited: "受限"
    },
    sidebar: {
      title: "扫描记录",
      empty: "暂无扫描"
    },
    composer: {
      label: "研究主题",
      placeholder: "例如：AI coding agents",
      submit: "启动雷达扫描",
      emptyError: "请输入研究主题",
      sourceScanFailed: "研究来源扫描失败",
      modelUnavailable: "模型分析暂不可用",
      filters: {
        papers: "论文",
        projects: "开源项目",
        datasets: "数据集",
        benchmarks: "基准",
        news: "新闻",
        community: "社区"
      },
      rankings: {
        balanced: "均衡",
        importance: "重要性",
        recency: "时效性",
        beginner: "新手"
      }
    },
    preview: {
      signal: "信号",
      signalValue: "主题热度",
      people: "人物",
      peopleValue: "作者与机构",
      experiments: "实验",
      experimentsValue: "下一步探索"
    },
    workspace: {
      sources: "来源",
      events: "事件",
      sections: "章节"
    },
    evidence: {
      title: "证据",
      waiting: "等待扫描",
      liveModelOutput: "模型实时输出",
      chunks: "片段",
      run: "运行",
      status: "状态",
      trace: "追踪"
    }
  },
  "en-US": {
    status: {
      ready: "ready",
      scanning: "scanning",
      failed: "failed",
      pending: "pending",
      running: "running",
      succeeded: "succeeded",
      limited: "limited"
    },
    sidebar: {
      title: "Scans",
      empty: "No scans"
    },
    composer: {
      label: "Research topic",
      placeholder: "Example: AI coding agents",
      submit: "Start radar scan",
      emptyError: "Enter a research topic",
      sourceScanFailed: "Research source scan failed",
      modelUnavailable: "model analysis unavailable",
      filters: {
        papers: "Papers",
        projects: "Projects",
        datasets: "Datasets",
        benchmarks: "Benchmarks",
        news: "News",
        community: "Community"
      },
      rankings: {
        balanced: "Balanced",
        importance: "Importance",
        recency: "Recency",
        beginner: "Beginner"
      }
    },
    preview: {
      signal: "Signal",
      signalValue: "topic velocity",
      people: "People",
      peopleValue: "authors and labs",
      experiments: "Experiments",
      experimentsValue: "next probes"
    },
    workspace: {
      sources: "sources",
      events: "events",
      sections: "sections"
    },
    evidence: {
      title: "Evidence",
      waiting: "Waiting for scan",
      liveModelOutput: "Live model output",
      chunks: "chunks",
      run: "run",
      status: "status",
      trace: "trace"
    }
  }
};

export function researchRadarCopy(locale: ResearchLocale): ResearchRadarCopy {
  return RESEARCH_RADAR_COPY[normalizeResearchLocale(locale)];
}
```

- [ ] **Step 4: Wire locale state and selector into the app**

In `apps/research-radar-poc/src/app-client.tsx`:

Import:

```ts
import {
  RESEARCH_LOCALE_OPTIONS,
  readSavedResearchLocale,
  researchRadarCopy,
  saveResearchLocale
} from "@/lib/i18n";
import type { ResearchLocale, ResearchRadarCopy } from "@/lib/i18n";
```

Add state near existing state:

```ts
  const [locale, setLocale] = useState<ResearchLocale>(() => readSavedResearchLocale());
  const copy = researchRadarCopy(locale);
```

Add handler:

```ts
  function handleLocaleSelect(nextLocale: ResearchLocale) {
    setLocale(nextLocale);
    saveResearchLocale(nextLocale);
  }
```

Pass `locale` into `createResearchRadarRun`:

```ts
        locale,
```

Replace hardcoded composer errors:

```ts
      setComposerError(copy.composer.emptyError);
```

```ts
          runError: sourceScan.warnings.join("\n") || copy.composer.sourceScanFailed
```

```ts
          ? copy.composer.modelUnavailable
```

Pass `copy`, `locale`, and `onLocaleSelect` into child components.

Add selector component:

```tsx
function LanguageSelector({
  locale,
  onSelect
}: {
  locale: ResearchLocale;
  onSelect: (locale: ResearchLocale) => void;
}) {
  return (
    <div className="flex rounded-[8px] border border-[#DDE5DD] bg-white p-1">
      {RESEARCH_LOCALE_OPTIONS.map((option) => (
        <button
          aria-pressed={locale === option.locale}
          className={[
            "h-8 rounded-[7px] px-3 text-[13px] font-medium transition",
            locale === option.locale
              ? "bg-[#17251F] text-white"
              : "text-[#66736B] hover:bg-[#F0F4F0] hover:text-[#1F2923]"
          ].join(" ")}
          key={option.locale}
          onClick={() => onSelect(option.locale)}
          type="button"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
```

Update `Header` to render a right-side group:

```tsx
      <div className="flex shrink-0 items-center gap-2">
        <LanguageSelector locale={locale} onSelect={onLocaleSelect} />
        <ModelSelector
          onSelect={onRouteSelect}
          options={modelOptions}
          selectedRouteId={selectedRouteId}
        />
      </div>
```

- [ ] **Step 5: Localize top-level app copy**

Replace:

- `Scans` with `copy.sidebar.title`
- `No scans` with `copy.sidebar.empty`
- `isSubmitting ? "scanning" : "ready"` with `isSubmitting ? copy.status.scanning : copy.status.ready`
- `failed` / `pending` with `copy.status.failed` / `copy.status.pending`
- filter labels from `FILTERS` with `copy.composer.filters[filter.code]`
- ranking labels from `RANKINGS` with `copy.composer.rankings[option.code]`
- preview cards with `copy.preview.*`
- metric labels with `copy.workspace.*`

- [ ] **Step 6: Run page tests to verify GREEN**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx
```

Expected: pass for default Chinese, English switching, saved locale, invalid locale, and existing scan flows.

- [ ] **Step 7: Commit Task 2**

```bash
git add apps/research-radar-poc/src/lib/i18n.ts \
  apps/research-radar-poc/src/app-client.tsx \
  apps/research-radar-poc/app/page.test.tsx
git commit -m "feat: add research radar language selector"
```

### Task 3: Localize Graph, Evidence Drawer, and Inspector Surfaces

**Files:**
- Modify: `apps/research-radar-poc/src/lib/i18n.ts`
- Modify: `apps/research-radar-poc/src/components/research-map.tsx`
- Modify: `apps/research-radar-poc/src/components/research-map.test.tsx`
- Modify: `apps/research-radar-poc/src/app-client.tsx`
- Modify: `apps/research-radar-poc/app/page.test.tsx`

**Interfaces:**
- Consumes:
  - `ResearchRadarCopy` from Task 2.
- Produces:
  - `ResearchMapCopy`
  - `ResearchMap` prop `copy?: ResearchMapCopy`
  - Chinese and English copy for graph labels, evidence drawer, source results, and node inspector.

- [ ] **Step 1: Write failing map copy tests**

In `apps/research-radar-poc/src/components/research-map.test.tsx`, add:

```ts
  it("uses supplied Chinese map copy", () => {
    render(
      <ResearchMap
        graph={graph}
        selectedNodeId={null}
        onNodeSelect={() => {}}
        copy={{
          title: "研究图谱",
          description: "探索主题、证据、空白与实验之间的联系。",
          graphLabel: "研究关系图",
          nodeCount: (count) => `${count} 个节点`,
          noUsableNodes: "暂无可用图谱节点",
          noUsableNodesDescription: "覆盖受限时，来源警告和限制会显示在下方。",
          caveats: "限制",
          layers: {
            papers: "论文",
            people: "人物",
            projects: "项目",
            models: "模型",
            datasets: "数据集",
            benchmarks: "基准",
            questions: "问题",
            experiments: "实验"
          },
          nodeKinds: {
            topic: "主题",
            hotspot: "热点",
            paper: "论文",
            project: "项目",
            model: "模型",
            dataset: "数据集",
            benchmark: "基准",
            author: "作者",
            institution: "机构",
            open_question: "开放问题",
            experiment: "实验"
          }
        }}
      />
    );

    expect(screen.getByText("研究图谱")).toBeTruthy();
    expect(screen.getByText("论文")).toBeTruthy();
    expect(screen.getByLabelText("研究关系图")).toBeTruthy();
  });
```

- [ ] **Step 2: Run map tests to verify RED**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/components/research-map.test.tsx
```

Expected: fail because `ResearchMap` has no `copy` prop.

- [ ] **Step 3: Add graph and evidence copy to i18n**

In `apps/research-radar-poc/src/lib/i18n.ts`, add local map-copy types. Keep these local to avoid a type import cycle between `src/types/research.ts` and `src/lib/i18n.ts`:

```ts
type ResearchMapNodeKind =
  | "topic"
  | "hotspot"
  | "paper"
  | "project"
  | "model"
  | "dataset"
  | "benchmark"
  | "author"
  | "institution"
  | "open_question"
  | "experiment";

export type ResearchMapCopy = {
  title: string;
  description: string;
  graphLabel: string;
  nodeCount: (count: number) => string;
  noUsableNodes: string;
  noUsableNodesDescription: string;
  caveats: string;
  layers: Record<"papers" | "people" | "projects" | "models" | "datasets" | "benchmarks" | "questions" | "experiments", string>;
  nodeKinds: Record<ResearchMapNodeKind, string>;
};
```

Extend `ResearchRadarCopy`:

```ts
  map: ResearchMapCopy;
  drawer: {
    title: string;
    description: string;
    sourceResults: string;
    sourceResultsDescription: string;
    waiting: string;
    items: (count: number) => string;
    noItems: string;
    noSummary: string;
  };
  inspector: {
    title: string;
    kind: string;
    importance: string;
    noSummary: string;
    connectedEvidence: string;
    sourceLinks: string;
    caveats: string;
    suggestedNextAction: string;
    selectNode: string;
  };
```

Use Chinese values:

```ts
map: {
  title: "研究图谱",
  description: "探索主题、证据、空白与实验之间的联系。",
  graphLabel: "研究关系图",
  nodeCount: (count) => `${count} 个节点`,
  noUsableNodes: "暂无可用图谱节点",
  noUsableNodesDescription: "覆盖受限时，来源警告和限制会显示在下方。",
  caveats: "限制",
  layers: {
    papers: "论文",
    people: "人物",
    projects: "项目",
    models: "模型",
    datasets: "数据集",
    benchmarks: "基准",
    questions: "问题",
    experiments: "实验"
  },
  nodeKinds: {
    topic: "主题",
    hotspot: "热点",
    paper: "论文",
    project: "项目",
    model: "模型",
    dataset: "数据集",
    benchmark: "基准",
    author: "作者",
    institution: "机构",
    open_question: "开放问题",
    experiment: "实验"
  }
},
drawer: {
  title: "证据抽屉",
  description: "原始 API 结果和来源警告",
  sourceResults: "来源结果",
  sourceResultsDescription: "模型报告前收集的 API 证据",
  waiting: "等待来源证据",
  items: (count) => `${count} 条`,
  noItems: "没有返回条目",
  noSummary: "暂无摘要"
},
inspector: {
  title: "节点详情",
  kind: "类型",
  importance: "重要性",
  noSummary: "暂无节点摘要。",
  connectedEvidence: "关联证据",
  sourceLinks: "来源链接",
  caveats: "限制",
  suggestedNextAction: "建议下一步",
  selectNode: "在研究图谱中选择一个节点"
}
```

Use English values equivalent to the current UI:

```ts
map: {
  title: "Research Map",
  description: "Explore how topics, evidence, gaps, and experiments connect.",
  graphLabel: "Research graph",
  nodeCount: (count) => `${count} nodes`,
  noUsableNodes: "No usable graph nodes",
  noUsableNodesDescription: "Source warnings and caveats are listed below when coverage is limited.",
  caveats: "Caveats",
  layers: {
    papers: "Papers",
    people: "People",
    projects: "Projects",
    models: "Models",
    datasets: "Datasets",
    benchmarks: "Benchmarks",
    questions: "Questions",
    experiments: "Experiments"
  },
  nodeKinds: {
    topic: "topic",
    hotspot: "hotspot",
    paper: "paper",
    project: "project",
    model: "model",
    dataset: "dataset",
    benchmark: "benchmark",
    author: "author",
    institution: "institution",
    open_question: "open question",
    experiment: "experiment"
  }
},
drawer: {
  title: "Evidence Drawer",
  description: "Raw API results and source warnings",
  sourceResults: "Source Results",
  sourceResultsDescription: "API evidence collected before the model report",
  waiting: "Waiting for source evidence",
  items: (count) => `${count} items`,
  noItems: "No items returned",
  noSummary: "No summary"
},
inspector: {
  title: "Node Inspector",
  kind: "kind",
  importance: "importance",
  noSummary: "No node summary available.",
  connectedEvidence: "Connected evidence",
  sourceLinks: "Source links",
  caveats: "Caveats",
  suggestedNextAction: "Suggested next action",
  selectNode: "Select a node in the research map"
}
```

- [ ] **Step 4: Update ResearchMap to accept localized copy**

In `apps/research-radar-poc/src/components/research-map.tsx`:

```ts
import type { ResearchMapCopy } from "@/lib/i18n";
```

Update props:

```ts
export type ResearchMapProps = {
  graph: ResearchGraph;
  selectedNodeId: string | null;
  onNodeSelect: (nodeId: string) => void;
  copy?: ResearchMapCopy;
};
```

Add default English map copy near constants:

```ts
const DEFAULT_MAP_COPY: ResearchMapCopy = {
  title: "Research Map",
  description: "Explore how topics, evidence, gaps, and experiments connect.",
  graphLabel: "Research graph",
  nodeCount: (count) => `${count} nodes`,
  noUsableNodes: "No usable graph nodes",
  noUsableNodesDescription: "Source warnings and caveats are listed below when coverage is limited.",
  caveats: "Caveats",
  layers: {
    papers: "Papers",
    people: "People",
    projects: "Projects",
    models: "Models",
    datasets: "Datasets",
    benchmarks: "Benchmarks",
    questions: "Questions",
    experiments: "Experiments"
  },
  nodeKinds: {
    topic: "topic",
    hotspot: "hotspot",
    paper: "paper",
    project: "project",
    model: "model",
    dataset: "dataset",
    benchmark: "benchmark",
    author: "author",
    institution: "institution",
    open_question: "open question",
    experiment: "experiment"
  }
};
```

Update function signature:

```tsx
export function ResearchMap({ graph, selectedNodeId, onNodeSelect, copy = DEFAULT_MAP_COPY }: ResearchMapProps) {
```

Replace hardcoded labels:

- `Research Map` with `copy.title`
- description with `copy.description`
- `aria-label="Research graph"` with `aria-label={copy.graphLabel}`
- `{graph.nodes.length} nodes` with `{copy.nodeCount(graph.nodes.length)}`
- layer labels with `copy.layers[layer.layer]`
- `node.kind.replaceAll("_", " ")` with `copy.nodeKinds[node.kind]`
- empty state with `copy.noUsableNodes` and `copy.noUsableNodesDescription`
- `Caveats:` with `{copy.caveats}:`

- [ ] **Step 5: Wire map, drawer, and inspector copy from app**

In `apps/research-radar-poc/src/app-client.tsx`:

- Pass `copy.map` into `ResearchMap`.
- Pass `copy.drawer` into `EvidenceDrawer` and `SourceResults`.
- Pass `copy.inspector` into `EvidenceRail`.
- Replace hardcoded `Evidence`, `Waiting for scan`, `Live model output`, `chunks`, `Node Inspector`, `No node summary available.`, `Connected evidence`, `Source links`, `Caveats`, `Suggested next action`, and `Select a node in the research map` with dictionary values.

Use this call shape:

```tsx
<ResearchMap
  copy={copy.map}
  graph={researchGraph}
  onNodeSelect={onGraphNodeSelect}
  selectedNodeId={selectedGraphNodeId}
/>
```

Update `EvidenceDrawer` signature:

```tsx
function EvidenceDrawer({
  copy,
  sources
}: {
  copy: ResearchRadarCopy["drawer"];
  sources: ResearchSourceResult[];
}) {
```

Update `EvidenceRail` props:

```ts
  copy: ResearchRadarCopy["evidence"];
  inspectorCopy: ResearchRadarCopy["inspector"];
```

- [ ] **Step 6: Add page-level assertions for localized graph surfaces**

In `apps/research-radar-poc/app/page.test.tsx`, update graph assertions:

```ts
    expect(await screen.findByText("研究图谱")).toBeTruthy();
    expect(screen.getByRole("button", { name: "证据抽屉" })).toBeTruthy();
```

In the node selection test, expect:

```ts
    expect(await screen.findByText("节点详情")).toBeTruthy();
    expect(await screen.findByText("关联证据")).toBeTruthy();
    expect(await screen.findByText("来源链接")).toBeTruthy();
    expect(await screen.findByText("建议下一步")).toBeTruthy();
```

Add English graph assertion after clicking English:

```ts
    expect(screen.getByText("Research Map")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Evidence Drawer" })).toBeTruthy();
```

- [ ] **Step 7: Run focused tests to verify GREEN**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx src/components/research-map.test.tsx
```

Expected: pass.

- [ ] **Step 8: Commit Task 3**

```bash
git add apps/research-radar-poc/src/lib/i18n.ts \
  apps/research-radar-poc/src/components/research-map.tsx \
  apps/research-radar-poc/src/components/research-map.test.tsx \
  apps/research-radar-poc/src/app-client.tsx \
  apps/research-radar-poc/app/page.test.tsx
git commit -m "feat: localize research radar graph surfaces"
```

### Task 4: Final Verification

**Files:**
- No new files.

**Interfaces:**
- Consumes all previous tasks.
- Produces verified i18n feature branch.

- [ ] **Step 1: Run full Research Radar POC tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx src/api/research.test.ts src/api/source-scan.test.ts src/lib/i18n.test.ts src/lib/research-graph.test.ts src/components/research-map.test.tsx
```

Expected: all tests pass.

- [ ] **Step 2: Run typecheck**

Run:

```bash
pnpm --dir apps/research-radar-poc typecheck
```

Expected: `tsc --noEmit` exits 0.

- [ ] **Step 3: Run lint**

Run:

```bash
pnpm --dir apps/research-radar-poc lint
```

Expected: `eslint .` exits 0.

- [ ] **Step 4: Run production build**

Run:

```bash
pnpm --dir apps/research-radar-poc build
```

Expected: Next build exits 0. If `apps/research-radar-poc/next-env.d.ts` changes due to Next generation, restore only that generated change with `apply_patch`.

- [ ] **Step 5: Check whitespace**

Run:

```bash
git diff --check
```

Expected: no output and exit 0.

- [ ] **Step 6: Check final status**

Run:

```bash
git status --short
```

Expected: only intentional i18n commits are on the branch; do not modify, stage, or revert `apps/codex-app-poc/app/layout.tsx`.
