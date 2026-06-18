import { createDataset, listDatasets } from "./knowledge";
import type { AgentRunCommand, WorkbenchContext } from "@/types/agent";
import type { DatasetResp } from "@/types/knowledge";

export const WORKBENCH_DATASET_NAME = "Codex Workbench Inbox";

const WORKBENCH_AGENT_BUDGET = {
  maxSteps: 8,
  maxToolCalls: 2,
  maxSeconds: 90,
  maxCostCents: 0
};

export function defaultWorkbenchContext(routeId?: string): WorkbenchContext {
  return {
    mode: "agent",
    documentIds: [],
    fileIds: [],
    skillCodes: [],
    mcpToolCodes: [],
    webSearchEnabled: false,
    ...(routeId ? { routeId } : {})
  };
}

export function buildWorkbenchAgentRunCommand(
  input: string,
  context: WorkbenchContext
): AgentRunCommand {
  const configuredRouteId = configuredAgentModelRouteId();
  const routeId = context.routeId?.trim() || configuredRouteId;
  const workbenchContext: WorkbenchContext = {
    ...context,
    ...(routeId ? { routeId } : {})
  };

  return {
    input,
    runtimeMode: "model_loop",
    autoApprove: false,
    ...(routeId ? { modelRouteId: routeId } : {}),
    budget: WORKBENCH_AGENT_BUDGET,
    workbenchContext
  };
}

export async function ensureWorkbenchDataset(): Promise<DatasetResp> {
  const existing = await listDatasets({ name: WORKBENCH_DATASET_NAME, page: 1, size: 10 });
  const matched = existing.list.find((dataset) => dataset.name === WORKBENCH_DATASET_NAME);
  if (matched) {
    return matched;
  }

  const id = await createDataset({
    name: WORKBENCH_DATASET_NAME,
    description: "Default uploaded-file inbox for the Codex conversation workbench POC.",
    visibility: 1,
    retrievalMode: 1
  });

  return {
    id,
    tenantId: 0,
    name: WORKBENCH_DATASET_NAME,
    description: "Default uploaded-file inbox for the Codex conversation workbench POC.",
    ownerId: 0,
    visibility: 1,
    status: 1,
    retrievalMode: 1,
    documentCount: 0,
    chunkCount: 0,
    createUserString: "",
    createTime: "",
    updateUserString: "",
    updateTime: ""
  };
}

function configuredAgentModelRouteId() {
  return (process.env.NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID ?? "").trim() || undefined;
}
