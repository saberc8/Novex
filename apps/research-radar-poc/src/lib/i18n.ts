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
    scanFailed: string;
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
    run: string;
    pendingModelOutput: string;
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
      scanFailed: "雷达扫描失败",
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
      sections: "章节",
      run: "运行",
      pendingModelOutput: "等待模型输出"
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
      scanFailed: "Radar scan failed",
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
      sections: "sections",
      run: "Run",
      pendingModelOutput: "Pending model output"
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
