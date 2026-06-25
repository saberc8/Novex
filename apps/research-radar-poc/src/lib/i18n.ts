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
  layers: Record<
    "papers" | "people" | "projects" | "models" | "datasets" | "benchmarks" | "questions" | "experiments",
    string
  >;
  nodeKinds: Record<ResearchMapNodeKind, string>;
};

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
  evidence: {
    title: string;
    waiting: string;
    liveModelOutput: string;
    chunks: string;
    run: string;
    status: string;
    trace: string;
  };
  inspector: {
    title: string;
    kind: string;
    importance: string;
    evidenceCount: (count: number) => string;
    noSummary: string;
    connectedEvidence: string;
    emptyConnectedEvidence: string;
    sourceLinks: string;
    emptySourceLinks: string;
    caveats: string;
    suggestedNextAction: string;
    selectNode: string;
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
    evidence: {
      title: "证据",
      waiting: "等待扫描",
      liveModelOutput: "模型实时输出",
      chunks: "片段",
      run: "运行",
      status: "状态",
      trace: "追踪"
    },
    inspector: {
      title: "节点详情",
      kind: "类型",
      importance: "重要性",
      evidenceCount: (count) => `证据 ${count}`,
      noSummary: "暂无节点摘要。",
      connectedEvidence: "关联证据",
      emptyConnectedEvidence: "暂无关联证据。",
      sourceLinks: "来源链接",
      emptySourceLinks: "暂无来源链接。",
      caveats: "限制",
      suggestedNextAction: "建议下一步",
      selectNode: "在研究图谱中选择一个节点"
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
    evidence: {
      title: "Evidence",
      waiting: "Waiting for scan",
      liveModelOutput: "Live model output",
      chunks: "chunks",
      run: "run",
      status: "status",
      trace: "trace"
    },
    inspector: {
      title: "Node Inspector",
      kind: "kind",
      importance: "importance",
      evidenceCount: (count) => `evidence ${count}`,
      noSummary: "No node summary available.",
      connectedEvidence: "Connected evidence",
      emptyConnectedEvidence: "No connected evidence yet.",
      sourceLinks: "Source links",
      emptySourceLinks: "No linked source URLs.",
      caveats: "Caveats",
      suggestedNextAction: "Suggested next action",
      selectNode: "Select a node in the research map"
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
