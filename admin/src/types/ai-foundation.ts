export type FoundationStatus = "skeleton";

export interface FoundationModuleResp {
  id: string;
  name: string;
  layer: string;
  status: FoundationStatus;
  description: string;
}

export interface FoundationSummaryResp {
  status: FoundationStatus;
  totalModules: number;
  modules: FoundationModuleResp[];
}
