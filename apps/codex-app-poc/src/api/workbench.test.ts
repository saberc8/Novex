import { afterEach, describe, expect, it, vi } from "vitest";
import { buildWorkbenchAgentRunCommand, ensureWorkbenchDataset } from "./workbench";

describe("workbench api helpers", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it("builds configured model-loop commands with typed workbench context", () => {
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", " runtime.llm.code_agent ");

    const command = buildWorkbenchAgentRunCommand("Summarize this file", {
      mode: "agent",
      datasetId: 7,
      documentIds: [11],
      fileIds: [19],
      skillCodes: ["support.refund"],
      mcpToolCodes: ["mcp.docs.search"],
      webSearchEnabled: true,
      routeId: "runtime.llm.code_agent"
    });

    expect(command).toEqual({
      input: "Summarize this file",
      runtimeMode: "model_loop",
      autoApprove: false,
      modelRouteId: "runtime.llm.code_agent",
      budget: {
        maxSteps: 8,
        maxToolCalls: 2,
        maxSeconds: 90,
        maxCostCents: 0
      },
      workbenchContext: {
        mode: "agent",
        datasetId: 7,
        documentIds: [11],
        fileIds: [19],
        skillCodes: ["support.refund"],
        mcpToolCodes: ["mcp.docs.search"],
        webSearchEnabled: true,
        routeId: "runtime.llm.code_agent"
      }
    });
  });

  it("reuses an existing Codex Workbench Inbox dataset", async () => {
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => ({
      ok: true,
      json: async () => {
        if (String(url).includes("/ai/knowledge/datasets?name=Codex+Workbench+Inbox")) {
          return {
            code: "200",
            data: {
              list: [{ id: 9, name: "Codex Workbench Inbox", status: 1 }],
              total: 1
            }
          };
        }
        throw new Error(`unexpected request ${url} ${init?.method}`);
      }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const dataset = await ensureWorkbenchDataset();

    expect(dataset.id).toBe(9);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
