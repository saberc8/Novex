import { createAgentRun } from "./agent";
import type { AgentRunCommand, AgentRunResp } from "@/types/agent";
import type { ModelRouteOption, ResearchFilter, ResearchRanking, ResearchScanInput } from "@/types/research";

const DEFAULT_MODEL_ROUTE_ID = "runtime.llm";

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

const REPORT_HEADINGS = [
  "## Research Overview",
  "## Active Topics",
  "## Key Authors And Institutions",
  "## Representative Work",
  "## Reading Route",
  "## Research Openings",
  "## Experiment Plans",
  "## Sources And Caveats"
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
  const filters = input.filters.length > 0
    ? input.filters.map((filter) => FILTER_LABELS[filter]).join(", ")
    : "Papers, Open source projects, Datasets, Benchmarks, News, Community discussion";
  const ranking = RANKING_LABELS[input.ranking];

  return [
    "You are an AI research radar for scientists entering a new research direction.",
    `Research topic: ${input.topic.trim()}`,
    `Focus sources: ${filters}`,
    `Ranking priority: ${ranking}`,
    sourceEvidencePrompt(input),
    "Use web search when useful. Prefer recent, source-grounded information, but clearly mark uncertainty, stale information, and missing coverage.",
    "Use at most 3 web search calls total. After those searches, synthesize the report with caveats instead of searching again.",
    "Return a concise markdown report with exactly these headings:",
    ...REPORT_HEADINGS,
    "For each section, include practical details that help a newcomer decide what to read, who to follow, what work matters, and which experiments are worth trying."
  ].join("\n");
}

function sourceEvidencePrompt(input: ResearchScanInput) {
  const evidence = input.sourceScan?.promptContext?.trim();
  if (!evidence) {
    return "No backend source evidence was provided. Use web search when useful.";
  }

  return [
    "Use the provided backend source evidence first. Use web search only to fill gaps or verify stale coverage.",
    evidence
  ].join("\n");
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
