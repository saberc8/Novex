import { createAgentRun } from "./agent";
import { normalizeResearchLocale, researchReportHeadings, researchReportLanguageInstruction } from "@/lib/i18n";
import type { AgentRunCommand, AgentRunResp } from "@/types/agent";
import type {
  ModelRouteOption,
  ResearchFilter,
  ResearchRanking,
  ResearchScanInput,
  ResearchSourceItem,
  ResearchTopicPlan
} from "@/types/research";

const DEFAULT_MODEL_ROUTE_ID = "runtime.llm";
const AGENT_INPUT_MAX_CHARS = 4000;
const SOURCE_EVIDENCE_TRUNCATION_NOTICE = "Source evidence truncated to fit Agent input limit.";

const RESEARCH_RADAR_BUDGET = {
  maxSteps: 10,
  maxToolCalls: 6,
  maxSeconds: 180,
  maxCostCents: 0
};

const TOPIC_PLANNER_BUDGET = {
  maxSteps: 3,
  maxToolCalls: 0,
  maxSeconds: 90,
  maxCostCents: 0
};

const REPORT_REPAIR_BUDGET = {
  maxSteps: 3,
  maxToolCalls: 1,
  maxSeconds: 90,
  maxCostCents: 0
};

type ResearchReportRepairInput = ResearchScanInput & {
  previousOutput: string;
};

const FILTER_LABELS: Record<ResearchFilter, string> = {
  papers: "Papers",
  projects: "Open source projects",
  datasets: "Datasets",
  benchmarks: "Benchmarks",
  news: "News",
  community: "Community discussion"
};

const RANKING_LABELS: Record<ResearchRanking, string> = {
  balanced: "Balanced",
  importance: "Importance",
  recency: "Recency",
  beginner: "Beginner friendly"
};

const SOURCE_LABELS: Record<ResearchSourceItem["source"], string> = {
  arxiv: "arXiv",
  github: "GitHub",
  huggingface_models: "HuggingFace Models",
  huggingface_datasets: "HuggingFace Datasets",
  paperswithcode: "PapersWithCode",
  leaderboards: "Leaderboards"
};

const GRAPH_JSON_INSTRUCTION = [
  "Before the markdown report, return one compact fenced graph block:",
  "```research-graph-json",
  '{ "topic": "...", "nodes": [], "edges": [], "caveats": [] }',
  "```",
  "Graph node kinds: topic, hotspot, paper, project, model, dataset, benchmark, author, institution, open_question, experiment.",
  "Graph edge relations: supports, implements, evaluates, extends, reveals_gap, leads_to, mentions.",
  "Keep graph JSON compact: at most 18 nodes and 28 edges."
];

export function buildResearchRadarAgentRunCommand(input: ResearchScanInput): AgentRunCommand {
  const routeId = input.routeId?.trim() || configuredAgentModelRouteId();

  return {
    input: buildResearchRadarPrompt(input),
    runtimeMode: "model_loop",
    autoApprove: false,
    ...(routeId ? { modelRouteId: routeId } : {}),
    budget: RESEARCH_RADAR_BUDGET,
    workbenchContext: {
      mode: "agent",
      documentIds: [],
      fileIds: [],
      skillCodes: [],
      mcpToolCodes: [],
      webSearchEnabled: true,
      ...(routeId ? { routeId } : {})
    }
  };
}

export function buildResearchTopicPlannerAgentRunCommand(input: ResearchScanInput): AgentRunCommand {
  const routeId = input.routeId?.trim() || configuredAgentModelRouteId();

  return {
    input: buildResearchTopicPlannerPrompt(input),
    runtimeMode: "model_loop",
    autoApprove: false,
    ...(routeId ? { modelRouteId: routeId } : {}),
    budget: TOPIC_PLANNER_BUDGET,
    workbenchContext: {
      mode: "agent",
      documentIds: [],
      fileIds: [],
      skillCodes: [],
      mcpToolCodes: [],
      webSearchEnabled: false,
      ...(routeId ? { routeId } : {})
    }
  };
}

export function buildResearchRadarRepairAgentRunCommand(input: ResearchReportRepairInput): AgentRunCommand {
  const routeId = input.routeId?.trim() || configuredAgentModelRouteId();

  return {
    input: buildResearchRadarRepairPrompt(input),
    runtimeMode: "model_loop",
    autoApprove: false,
    ...(routeId ? { modelRouteId: routeId } : {}),
    budget: REPORT_REPAIR_BUDGET,
    workbenchContext: {
      mode: "agent",
      documentIds: [],
      fileIds: [],
      skillCodes: [],
      mcpToolCodes: [],
      webSearchEnabled: false,
      ...(routeId ? { routeId } : {})
    }
  };
}

export function configuredModelRouteOptions(): ModelRouteOption[] {
  const configured = configuredAgentModelRouteId();
  const rawOptions = (process.env.NEXT_PUBLIC_AGENT_MODEL_ROUTE_OPTIONS ?? "").trim();
  const options = rawOptions
    .split(",")
    .map((item) => {
      const [routeId, label] = item.split(":");
      const normalizedRouteId = routeId?.trim();
      if (!normalizedRouteId) {
        return null;
      }
      return {
        routeId: normalizedRouteId,
        label: label?.trim() || normalizedRouteId
      };
    })
    .filter((option): option is ModelRouteOption => option !== null);

  const fallback = configured ? [{ routeId: configured, label: configured }] : [];
  const candidates = options.length > 0 ? options : fallback;
  const deduped = uniqueRouteOptions(candidates.length > 0 ? candidates : [
    { routeId: DEFAULT_MODEL_ROUTE_ID, label: DEFAULT_MODEL_ROUTE_ID }
  ]);

  if (!configured) {
    return deduped;
  }

  return [
    ...deduped.filter((option) => option.routeId === configured),
    ...deduped.filter((option) => option.routeId !== configured)
  ];
}

export async function createResearchRadarRun(input: ResearchScanInput): Promise<AgentRunResp> {
  return createAgentRun(buildResearchRadarAgentRunCommand(input));
}

export async function createResearchRadarRepairRun(input: ResearchReportRepairInput): Promise<AgentRunResp> {
  return createAgentRun(buildResearchRadarRepairAgentRunCommand(input));
}

export function buildResearchRadarFallbackReport(input: ResearchScanInput): string {
  const locale = normalizeResearchLocale(input.locale);
  const headings = researchReportHeadings(locale);
  const topic = input.topic.trim() || input.topicPlan?.topic || "research topic";
  const sourceItems = uniqueSourceItems([
    ...(input.sourceScan?.items ?? []),
    ...(input.sourceScan?.sources.flatMap((source) => source.items) ?? [])
  ]);
  const representativeItems = sourceItems.slice(0, 6);
  const warnings = uniqueStrings([
    ...(input.sourceScan?.warnings ?? []),
    ...(input.sourceScan?.sources.flatMap((source) => (source.warning ? [source.warning] : [])) ?? [])
  ]);
  const concepts = uniqueStrings([
    ...(input.topicPlan?.keyConcepts ?? []),
    ...(input.topicPlan?.domains ?? []),
    ...representativeItems.flatMap((item) => item.tags)
  ]).slice(0, 8);
  const itemLines = representativeItems.map((item) =>
    `- ${formatSourceItem(item)}${item.summary ? `：${previewText(item.summary, 120)}` : ""}`
  );
  const graph = fallbackGraph(topic, representativeItems, warnings);
  const content = fallbackReportContent(locale, {
    topic,
    headings,
    summary: input.topicPlan?.summary,
    learningGoals: input.topicPlan?.learningGoals ?? [],
    concepts,
    itemLines,
    warnings: summarizeWarningsForReport(warnings, locale)
  });

  return [
    "```research-graph-json",
    JSON.stringify(graph),
    "```",
    ...content
  ].join("\n");
}

export async function createResearchTopicPlan(input: ResearchScanInput): Promise<{
  plan: ResearchTopicPlan;
  run: AgentRunResp;
}> {
  const run = await createAgentRun(buildResearchTopicPlannerAgentRunCommand(input));
  return {
    plan: parseResearchTopicPlanFromRun(run, input.topic, input.filters),
    run
  };
}

export function parseResearchTopicPlanFromRun(
  run: Pick<AgentRunResp, "finalOutput">,
  fallbackTopic = "research topic",
  fallbackFilters: ResearchFilter[] = []
): ResearchTopicPlan {
  const rawJson = extractFencedBlock(run.finalOutput ?? "", "research-topic-plan-json");
  if (!rawJson) {
    return fallbackTopicPlan(fallbackTopic, fallbackFilters);
  }

  try {
    return normalizeTopicPlan(JSON.parse(rawJson), fallbackTopic, fallbackFilters);
  } catch {
    return fallbackTopicPlan(fallbackTopic, fallbackFilters);
  }
}

export function buildFallbackResearchTopicPlan(topic: string, filters: ResearchFilter[] = []): ResearchTopicPlan {
  return fallbackTopicPlan(topic, filters);
}

function buildResearchTopicPlannerPrompt(input: ResearchScanInput) {
  const locale = normalizeResearchLocale(input.locale);
  const filters = input.filters.length > 0
    ? input.filters.map((filter) => FILTER_LABELS[filter]).join(", ")
    : "Papers, Open source projects, Datasets, Benchmarks, News, Community discussion";
  const ranking = RANKING_LABELS[input.ranking];

  return fitTextToBudget(
    [
      "You are a topic planner for a general-purpose AI research radar.",
      "Do not browse or call tools. Analyze the term itself before any source search.",
      `Research topic: ${input.topic.trim()}`,
      `Requested source types: ${filters}`,
      `Ranking priority: ${ranking}`,
      researchReportLanguageInstruction(locale),
      "Return exactly one compact fenced JSON block and no markdown prose:",
      "```research-topic-plan-json",
      JSON.stringify({
        topic: input.topic.trim(),
        summary: "one sentence explaining what a newcomer needs to understand",
        domains: ["domain or discipline"],
        learningGoals: ["what the user should learn first"],
        keyConcepts: ["core concept or term"],
        searchQueries: ["query in original language", "English synonym query"],
        relevanceKeywords: ["keyword used to reject off-topic search results"],
        sourcePriorities: input.filters.length > 0 ? input.filters : ["papers", "projects", "datasets", "benchmarks"]
      }),
      "```",
      "Rules:",
      "- Produce 2-4 domains, 4-8 learningGoals, 6-12 keyConcepts, 6-10 searchQueries, and 8-16 relevanceKeywords.",
      "- Include English aliases/translations when the topic is not English.",
      "- Search queries must be concrete enough to find papers, code, datasets, benchmarks, tutorials, or community discussions.",
      "- Relevance keywords must help filter out popular but unrelated results.",
      "- sourcePriorities must only use: papers, projects, datasets, benchmarks, news, community."
    ].join("\n"),
    AGENT_INPUT_MAX_CHARS
  );
}

function buildResearchRadarPrompt(input: ResearchScanInput) {
  const locale = normalizeResearchLocale(input.locale);
  const reportHeadings = researchReportHeadings(locale);
  const filters = input.filters.length > 0
    ? input.filters.map((filter) => FILTER_LABELS[filter]).join(", ")
    : "Papers, Open source projects, Datasets, Benchmarks, News, Community discussion";
  const ranking = RANKING_LABELS[input.ranking];
  const beforeEvidence = [
    "You are an AI research radar for scientists entering a new research direction.",
    `Research topic: ${input.topic.trim()}`,
    `Focus sources: ${filters}`,
    `Ranking priority: ${ranking}`,
    researchReportLanguageInstruction(locale),
    topicPlanPrompt(input.topicPlan)
  ];
  const afterEvidence = [
    "Use web search when useful. Prefer recent, source-grounded information, but clearly mark uncertainty, stale information, and missing coverage.",
    "Use at most 3 web search calls total. After those searches, synthesize the report with caveats instead of searching again.",
    "Do not return raw tool_call JSON in the final answer. If another search would help but the budget is exhausted, write the best grounded report with explicit caveats.",
    ...GRAPH_JSON_INSTRUCTION,
    "Return a concise markdown report with exactly these headings:",
    ...reportHeadings,
    "For each section, include practical details that help a newcomer decide what to learn, what to read, who to follow, what work matters, how to evaluate it, and which experiments are worth trying."
  ];
  const evidenceBudget = AGENT_INPUT_MAX_CHARS
    - beforeEvidence.join("\n").length
    - afterEvidence.join("\n").length
    - 2;

  return [
    ...beforeEvidence,
    sourceEvidencePrompt(input, evidenceBudget),
    ...afterEvidence
  ].join("\n");
}

function buildResearchRadarRepairPrompt(input: ResearchReportRepairInput) {
  const locale = normalizeResearchLocale(input.locale);
  const reportHeadings = researchReportHeadings(locale);
  const beforeEvidence = [
    "Repair invalid Research Radar report.",
    "Do not browse. Do not call tools. Do not output tool_call JSON.",
    "Rewrite the report now using only the provided topic plan, source evidence, caveats, and previous invalid output.",
    `Research topic: ${input.topic.trim()}`,
    `Ranking priority: ${RANKING_LABELS[input.ranking]}`,
    researchReportLanguageInstruction(locale),
    topicPlanPrompt(input.topicPlan)
  ];
  const afterEvidence = [
    "Previous invalid model output:",
    fitTextToBudget(input.previousOutput.trim() || "n/a", 650),
    ...GRAPH_JSON_INSTRUCTION,
    "Return a complete markdown report with exactly these headings:",
    ...reportHeadings,
    "Every heading needs practical, source-grounded content. If coverage is missing, state the limitation in the final section instead of calling tools."
  ];
  const evidenceBudget = AGENT_INPUT_MAX_CHARS
    - beforeEvidence.join("\n").length
    - afterEvidence.join("\n").length
    - 2;

  return [
    ...beforeEvidence,
    sourceEvidencePrompt(input, evidenceBudget),
    ...afterEvidence
  ].join("\n");
}

type FallbackReportContentInput = {
  topic: string;
  headings: string[];
  summary?: string;
  learningGoals: string[];
  concepts: string[];
  itemLines: string[];
  warnings: string[];
};

function fallbackReportContent(locale: ReturnType<typeof normalizeResearchLocale>, input: FallbackReportContentInput) {
  if (locale === "en-US") {
    return [
      input.headings[0],
      `Deterministic fallback analysis generated from collected evidence. ${input.summary ?? `Use the evidence base to map ${input.topic}.`}`,
      input.headings[1],
      bulletList(input.concepts, ["Evidence coverage, reproducible implementation, datasets, and evaluation protocol."]),
      input.headings[2],
      "Use the source list to identify project maintainers, paper authors, and institutions; verify authority before treating any actor as central.",
      input.headings[3],
      bulletList(input.itemLines, [`No representative source items were returned for ${input.topic}.`]),
      input.headings[4],
      bulletList(input.learningGoals, [
        "Clarify the core terms and assumptions.",
        "Read representative papers and inspect active open-source implementations.",
        "Reproduce one small experiment before expanding the search space."
      ]),
      input.headings[5],
      bulletList([
        `Compare source-backed claims around ${input.topic} instead of relying on a single provider.`,
        "Treat missing benchmark or leaderboard coverage as an explicit research gap."
      ], []),
      input.headings[6],
      bulletList([
        "Build a minimal reproduction from the strongest open-source project or dataset.",
        "Record metrics, assumptions, and failure cases before ranking alternatives."
      ], []),
      input.headings[7],
      bulletList(input.warnings, ["Model output was incomplete; this fallback report uses collected source evidence only."])
    ];
  }

  return [
    input.headings[0],
    `基于已收集证据生成的兜底分析。${input.summary ?? `当前应先围绕 ${input.topic} 建立概念、证据和实验框架。`}`,
    input.headings[1],
    bulletList(input.concepts, ["证据覆盖、可复现实作、数据集、评测协议与局限性。"]),
    input.headings[2],
    "优先从来源列表识别项目维护者、论文作者和机构；在引用为关键人物前，需要回到原始链接验证其贡献。",
    input.headings[3],
    bulletList(input.itemLines, [`暂未返回 ${input.topic} 的代表性来源条目。`]),
    input.headings[4],
    bulletList(input.learningGoals, [
      "先澄清核心术语、前提假设和适用边界。",
      "阅读代表性论文，同时查看仍活跃的开源实现。",
      "先复现一个小实验，再扩大检索范围和评测维度。"
    ]),
      input.headings[5],
      bulletList([
        `围绕 ${input.topic} 对比不同来源支持的结论，避免只依赖单一平台。`,
        "把缺失的 benchmark、leaderboard 或数据覆盖视为显式研究空白。"
      ], []),
      input.headings[6],
      bulletList([
        "选一个最相关开源项目或数据集做最小复现。",
        "记录指标、样本区间、假设、失败案例和可迁移性，再比较不同方案。"
      ], []),
    input.headings[7],
    bulletList(input.warnings, ["模型输出不完整；本报告仅基于已收集的来源证据生成。"])
  ];
}

function fallbackGraph(topic: string, items: ResearchSourceItem[], warnings: string[]) {
  const topicId = `topic:${slug(topic)}`;
  const sourceNodes = items.slice(0, 10).map((item, index) => ({
    id: `source:${slug(item.id) || index}`,
    kind: graphKindForSourceItem(item),
    title: item.title,
    summary: item.summary ?? item.organization ?? item.authors.slice(0, 3).join(", "),
    importance: Math.max(0.35, Math.min(1, metricTotal(item) > 0 ? Math.log10(metricTotal(item) + 1) / 5 : 0.45)),
    recency: item.publishedAt ?? item.updatedAt ?? null,
    sourceItemIds: [item.id],
    tags: item.tags
  }));

  return {
    topic,
    nodes: [
      {
        id: topicId,
        kind: "topic",
        title: topic,
        summary: "Central research point for this radar scan.",
        importance: 1,
        sourceItemIds: [],
        tags: []
      },
      ...sourceNodes
    ],
    edges: sourceNodes.map((node) => ({
      id: `${topicId}->${node.id}:mentions`,
      from: topicId,
      to: node.id,
      relation: "mentions",
      evidenceItemIds: node.sourceItemIds
    })),
    caveats: warnings
  };
}

function graphKindForSourceItem(item: ResearchSourceItem) {
  if (item.kind === "project" || item.kind === "model" || item.kind === "dataset" || item.kind === "benchmark") {
    return item.kind;
  }
  return "paper";
}

function summarizeWarningsForReport(warnings: string[], locale: ReturnType<typeof normalizeResearchLocale>) {
  return warnings.map((warning) => {
    if (warning.includes("Papers With Code-compatible endpoint")) {
      return locale === "en-US"
        ? "Papers With Code coverage is unavailable because the compatible endpoint is not configured."
        : "Papers With Code 覆盖不可用：兼容 endpoint 尚未配置。";
    }
    if (warning.includes("leaderboard endpoints are not configured")) {
      return locale === "en-US"
        ? "Leaderboard coverage is unavailable because leaderboard endpoints are not configured."
        : "榜单覆盖不可用：leaderboard endpoint 尚未配置。";
    }
    return warning;
  });
}

function formatSourceItem(item: ResearchSourceItem) {
  const suffixes = [
    SOURCE_LABELS[item.source],
    item.organization,
    item.metrics.length > 0
      ? item.metrics.slice(0, 2).map((metric) => `${metric.label} ${Math.round(metric.value)}`).join(", ")
      : null
  ].filter(Boolean);
  return `${item.title}${suffixes.length > 0 ? ` (${suffixes.join("; ")})` : ""}`;
}

function bulletList(items: string[], fallback: string[]) {
  return (items.length > 0 ? items : fallback)
    .slice(0, 8)
    .map((item) => item.startsWith("- ") ? item : `- ${item}`)
    .join("\n");
}

function uniqueSourceItems(items: ResearchSourceItem[]) {
  const seen = new Set<string>();
  const result: ResearchSourceItem[] = [];
  for (const item of items) {
    if (seen.has(item.id)) {
      continue;
    }
    seen.add(item.id);
    result.push(item);
  }
  return result;
}

function metricTotal(item: ResearchSourceItem) {
  return item.metrics.reduce((total, metric) => total + metric.value, 0);
}

function previewText(text: string, maxChars: number) {
  const trimmed = text.trim();
  return trimmed.length <= maxChars ? trimmed : `${trimmed.slice(0, maxChars - 1).trimEnd()}…`;
}

function slug(value: string) {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9\u4e00-\u9fa5]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80);
}

function topicPlanPrompt(topicPlan: ResearchTopicPlan | null | undefined) {
  if (!topicPlan) {
    return "Topic planner: no pre-scan topic plan was provided.";
  }

  return fitTextToBudget(
    [
      "Topic planner:",
      `Summary: ${topicPlan.summary}`,
      `Domains: ${formatPlanList(topicPlan.domains, 4)}`,
      `Learning goals: ${formatPlanList(topicPlan.learningGoals, 8)}`,
      `Key concepts: ${formatPlanList(topicPlan.keyConcepts, 12)}`,
      `Search queries: ${formatPlanList(topicPlan.searchQueries, 10)}`,
      `Relevance keywords: ${formatPlanList(topicPlan.relevanceKeywords, 16)}`
    ].join("\n"),
    900
  );
}

function formatPlanList(items: string[], limit: number) {
  return items.slice(0, limit).join("; ") || "n/a";
}

function sourceEvidencePrompt(input: ResearchScanInput, maxChars: number) {
  if (maxChars <= 0) {
    return "";
  }

  const evidence = input.sourceScan?.promptContext?.trim();
  if (!evidence) {
    return fitTextToBudget(
      "No backend source evidence was provided. Use web search when useful.",
      maxChars
    );
  }
  const intro = "Use the provided backend source evidence first. Use web search only to fill gaps or verify stale coverage.";
  if (maxChars <= intro.length) {
    return fitTextToBudget(intro, maxChars);
  }
  const bodyBudget = maxChars - intro.length - 1;

  return [
    intro,
    fitTextToBudget(evidence, bodyBudget)
  ].join("\n");
}

function fitTextToBudget(text: string, maxChars: number) {
  if (maxChars <= 0) {
    return "";
  }
  if (text.length <= maxChars) {
    return text;
  }

  const suffix = `\n${SOURCE_EVIDENCE_TRUNCATION_NOTICE}`;
  if (maxChars <= suffix.length) {
    return SOURCE_EVIDENCE_TRUNCATION_NOTICE.slice(0, maxChars);
  }

  return `${text.slice(0, maxChars - suffix.length).trimEnd()}${suffix}`;
}

function extractFencedBlock(markdown: string, fenceName: string) {
  const pattern = new RegExp(`\`\`\`${fenceName}\\s*([\\s\\S]*?)\\s*\`\`\``);
  return markdown.match(pattern)?.[1]?.trim() ?? null;
}

function normalizeTopicPlan(
  value: unknown,
  fallbackTopic: string,
  fallbackFilters: ResearchFilter[]
): ResearchTopicPlan {
  const record = isRecord(value) ? value : {};
  const topic = stringValue(record.topic) || fallbackTopic.trim() || "research topic";

  return {
    topic,
    summary: stringValue(record.summary) || `Plan searches and reading around ${topic}.`,
    domains: stringArray(record.domains, [topic]).slice(0, 4),
    learningGoals: stringArray(record.learningGoals, [`Understand the core concepts behind ${topic}.`]).slice(0, 8),
    keyConcepts: stringArray(record.keyConcepts, [topic]).slice(0, 12),
    searchQueries: stringArray(record.searchQueries, [topic]).slice(0, 10),
    relevanceKeywords: stringArray(record.relevanceKeywords, [topic]).slice(0, 16),
    sourcePriorities: filterArray(record.sourcePriorities, fallbackFilters)
  };
}

function fallbackTopicPlan(topic: string, filters: ResearchFilter[]): ResearchTopicPlan {
  const normalizedTopic = topic.trim() || "research topic";
  const domainHint = fallbackDomainHint(normalizedTopic);
  const domains = uniqueStrings([normalizedTopic, ...domainHint.domains]).slice(0, 4);
  const searchQueries = uniqueStrings([
    normalizedTopic,
    ...domainHint.searchQueries,
    `${normalizedTopic} research paper`,
    `${normalizedTopic} github open source`,
    `${normalizedTopic} dataset benchmark`,
    `${normalizedTopic} survey tutorial`
  ]).slice(0, 10);
  const relevanceKeywords = uniqueStrings([
    normalizedTopic,
    ...topicTokens(normalizedTopic),
    ...domainHint.relevanceKeywords
  ]).slice(0, 16);

  return {
    topic: normalizedTopic,
    summary: `Use source-oriented fallback queries to map concepts, papers, code, datasets, and benchmarks for ${normalizedTopic}.`,
    domains,
    learningGoals: [
      `Understand the core concepts and practical entry points for ${normalizedTopic}.`,
      `Find representative papers, open-source implementations, datasets, and benchmarks for ${normalizedTopic}.`,
      "Identify evaluation criteria, limitations, and a reproducible first experiment."
    ],
    keyConcepts: uniqueStrings([normalizedTopic, ...domainHint.keyConcepts]).slice(0, 12),
    searchQueries,
    relevanceKeywords,
    sourcePriorities: filters.length > 0 ? filters : ["papers", "projects", "datasets", "benchmarks"]
  };
}

function fallbackDomainHint(topic: string) {
  if (/量化|因子|股票|投资|金融|alpha/i.test(topic)) {
    return {
      domains: ["量化投资", "金融工程", "机器学习"],
      keyConcepts: ["alpha factor", "factor model", "IC", "backtest", "neutralization", "portfolio"],
      searchQueries: [
        "quant factor investing",
        "alpha factor model",
        "qlib factor research",
        "factor investing dataset benchmark",
        "quantitative investment alpha factor backtest"
      ],
      relevanceKeywords: ["factor", "alpha", "quant", "finance", "investment", "backtest", "IC", "portfolio", "qlib"]
    };
  }

  return {
    domains: ["research", "open source", "benchmark"],
    keyConcepts: [topic, "survey", "benchmark", "dataset", "implementation"],
    searchQueries: [],
    relevanceKeywords: ["paper", "github", "dataset", "benchmark", "survey"]
  };
}

function topicTokens(topic: string) {
  return topic
    .split(/[\s,，;；:/\\|()[\]{}"'`]+/)
    .map((token) => token.trim())
    .filter((token) => token.length >= 2);
}

function uniqueStrings(items: string[]) {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of items) {
    const normalized = item.trim();
    const key = normalized.toLowerCase();
    if (!normalized || seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(normalized);
  }
  return result;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function stringArray(value: unknown, fallback: string[]): string[] {
  if (!Array.isArray(value)) {
    return fallback;
  }
  const items = value
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter(Boolean);
  return items.length > 0 ? [...new Set(items)] : fallback;
}

function filterArray(value: unknown, fallback: ResearchFilter[]): ResearchFilter[] {
  const allowed: ResearchFilter[] = ["papers", "projects", "datasets", "benchmarks", "news", "community"];
  const items = Array.isArray(value)
    ? value.filter((item): item is ResearchFilter => allowed.includes(item as ResearchFilter))
    : [];
  const result = items.length > 0 ? items : fallback;
  return result.length > 0 ? [...new Set(result)] : ["papers", "projects", "datasets", "benchmarks"];
}

function configuredAgentModelRouteId() {
  return (process.env.NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID ?? "").trim() || undefined;
}

function uniqueRouteOptions(options: ModelRouteOption[]) {
  const seen = new Set<string>();
  return options.filter((option) => {
    if (seen.has(option.routeId)) {
      return false;
    }
    seen.add(option.routeId);
    return true;
  });
}
