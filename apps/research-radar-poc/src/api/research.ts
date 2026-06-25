import { createAgentRun } from "./agent";
import { normalizeResearchLocale, researchReportHeadings, researchReportLanguageInstruction } from "@/lib/i18n";
import type { AgentRunCommand, AgentRunResp } from "@/types/agent";
import type { ModelRouteOption, ResearchFilter, ResearchRanking, ResearchScanInput } from "@/types/research";

const DEFAULT_MODEL_ROUTE_ID = "runtime.llm";
const AGENT_INPUT_MAX_CHARS = 4000;
const SOURCE_EVIDENCE_TRUNCATION_NOTICE = "Source evidence truncated to fit Agent input limit.";

const RESEARCH_RADAR_BUDGET = {
  maxSteps: 10,
  maxToolCalls: 6,
  maxSeconds: 180,
  maxCostCents: 0
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
    researchReportLanguageInstruction(locale)
  ];
  const afterEvidence = [
    "Use web search when useful. Prefer recent, source-grounded information, but clearly mark uncertainty, stale information, and missing coverage.",
    "Use at most 3 web search calls total. After those searches, synthesize the report with caveats instead of searching again.",
    ...GRAPH_JSON_INSTRUCTION,
    "Return a concise markdown report with exactly these headings:",
    ...reportHeadings,
    "For each section, include practical details that help a newcomer decide what to read, who to follow, what work matters, and which experiments are worth trying."
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

function sourceEvidencePrompt(input: ResearchScanInput, maxChars: number) {
  const evidence = input.sourceScan?.promptContext?.trim();
  if (!evidence) {
    return fitTextToBudget(
      "No backend source evidence was provided. Use web search when useful.",
      maxChars
    );
  }
  const intro = "Use the provided backend source evidence first. Use web search only to fill gaps or verify stale coverage.";
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
