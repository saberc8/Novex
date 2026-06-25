import type { AgentRunEventResp, AgentRunResp } from "./agent";

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
};

export type ResearchScan = ResearchScanInput & {
  id: string;
  runResult: AgentRunResp | null;
  runEvents: AgentRunEventResp[];
  runError: string | null;
  createdAt: number;
};

export type ModelRouteOption = {
  routeId: string;
  label: string;
};
