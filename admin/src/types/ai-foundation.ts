export type FoundationStatus = "skeleton";
export type FoundationMilestoneStatus = "poc_ready" | "poc_limited" | "planned";

export interface FoundationModuleResp {
  id: string;
  name: string;
  layer: string;
  status: FoundationStatus;
  description: string;
}

export interface FoundationMilestoneCoverageResp {
  id: string;
  name: string;
  status: FoundationMilestoneStatus;
  summary: string;
  evidence: string[];
  limitations: string[];
}

export interface FoundationSummaryResp {
  status: FoundationStatus;
  totalModules: number;
  modules: FoundationModuleResp[];
  milestoneCoverage: FoundationMilestoneCoverageResp[];
}
