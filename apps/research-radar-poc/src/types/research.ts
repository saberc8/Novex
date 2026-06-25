import type { AgentRunEventResp, AgentRunResp } from "./agent";
import type { ResearchLocale } from "@/lib/i18n";

export type ResearchRanking = "balanced" | "importance" | "recency" | "beginner";

export type ResearchFilter =
  | "papers"
  | "projects"
  | "datasets"
  | "benchmarks"
  | "news"
  | "community";

export type ResearchScanInput = {
  topic: string;
  filters: ResearchFilter[];
  ranking: ResearchRanking;
  routeId?: string;
  locale?: ResearchLocale;
  sourceScan?: ResearchSourceScanResp | null;
};

export type ResearchAppErrorCode =
  | "empty_topic"
  | "source_scan_failed"
  | "model_unavailable"
  | "scan_failed";

export type ResearchUiError =
  | {
      kind: "app";
      code: ResearchAppErrorCode;
    }
  | {
      kind: "raw";
      message: string;
    };

export type ResearchScan = ResearchScanInput & {
  id: string;
  runResult: AgentRunResp | null;
  runEvents: AgentRunEventResp[];
  runError: ResearchUiError | null;
  createdAt: number;
};

export type ModelRouteOption = {
  routeId: string;
  label: string;
};

export type ResearchReportSection = {
  id: string;
  title: string;
  content: string;
};

export type ParsedResearchReport = {
  structured: boolean;
  sections: ResearchReportSection[];
};

export type ResearchGraphNodeKind =
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

export type ResearchGraphRelation =
  | "supports"
  | "implements"
  | "evaluates"
  | "extends"
  | "reveals_gap"
  | "leads_to"
  | "mentions";

export type ResearchGraphLayer =
  | "papers"
  | "projects"
  | "models"
  | "datasets"
  | "benchmarks"
  | "questions"
  | "experiments";

export type ResearchGraphNode = {
  id: string;
  kind: ResearchGraphNodeKind;
  title: string;
  summary: string;
  importance: number;
  recency?: string | null;
  sourceItemIds: string[];
  tags: string[];
};

export type ResearchGraphEdge = {
  id: string;
  from: string;
  to: string;
  relation: ResearchGraphRelation;
  evidenceItemIds: string[];
};

export type ResearchGraph = {
  topic: string;
  nodes: ResearchGraphNode[];
  edges: ResearchGraphEdge[];
  caveats: string[];
};

export type ResearchSource =
  | "arxiv"
  | "github"
  | "huggingface_models"
  | "huggingface_datasets"
  | "paperswithcode"
  | "leaderboards";

export type ResearchSourceScanInput = {
  topic: string;
  filters: ResearchFilter[];
  ranking: ResearchRanking;
};

export type ResearchSourceScanStatus = "succeeded" | "partial" | "failed";
export type ResearchSourceStatus = "succeeded" | "failed" | "degraded";
export type ResearchSourceItemKind =
  | "paper"
  | "project"
  | "model"
  | "dataset"
  | "benchmark"
  | "news"
  | "community";

export type ResearchSourceMetric = {
  label: string;
  value: number;
};

export type ResearchSourceItem = {
  id: string;
  source: ResearchSource;
  kind: ResearchSourceItemKind;
  title: string;
  url?: string | null;
  summary?: string | null;
  authors: string[];
  organization?: string | null;
  publishedAt?: string | null;
  updatedAt?: string | null;
  metrics: ResearchSourceMetric[];
  tags: string[];
  metadata: unknown;
};

export type ResearchSourceResult = {
  source: ResearchSource;
  status: ResearchSourceStatus;
  items: ResearchSourceItem[];
  warning?: string | null;
};

export type ResearchSourceScanResp = {
  topic: string;
  ranking: ResearchRanking;
  status: ResearchSourceScanStatus;
  sources: ResearchSourceResult[];
  items: ResearchSourceItem[];
  promptContext: string;
  warnings: string[];
};
