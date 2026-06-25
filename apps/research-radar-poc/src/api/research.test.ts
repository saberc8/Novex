import { describe, expect, it, vi } from "vitest";
import {
  buildResearchRadarAgentRunCommand,
  configuredModelRouteOptions,
  createResearchRadarRun
} from "./research";

describe("research radar agent command", () => {
  it("builds a model loop command with web search enabled", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "LLM agent memory",
      filters: ["papers", "projects", "benchmarks"],
      ranking: "recency",
      routeId: "runtime.llm.code_agent"
    });

    expect(command.runtimeMode).toBe("model_loop");
    expect(command.autoApprove).toBe(false);
    expect(command.modelRouteId).toBe("runtime.llm.code_agent");
    expect(command.budget).toEqual({
      maxSteps: 10,
      maxToolCalls: 6,
      maxSeconds: 180,
      maxCostCents: 0
    });
    expect(command.workbenchContext).toEqual({
      mode: "agent",
      documentIds: [],
      fileIds: [],
      skillCodes: [],
      mcpToolCodes: [],
      webSearchEnabled: true,
      routeId: "runtime.llm.code_agent"
    });
    expect(command.input).toContain("LLM agent memory");
    expect(command.input).toContain("Papers, Open source projects, Benchmarks");
    expect(command.input).toContain("Recency");
    expect(command.input).toContain("Use at most 3 web search calls total");
    expect(command.input).toContain("synthesize the report with caveats instead of searching again");
    expect(command.input).toContain("## Research Overview");
    expect(command.input).toContain("## Sources And Caveats");
  });

  it("uses configured route options from environment", () => {
    vi.stubEnv(
      "NEXT_PUBLIC_AGENT_MODEL_ROUTE_OPTIONS",
      "runtime.llm:Default Radar,runtime.llm.deep:Deep Research"
    );
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", "runtime.llm.deep");

    expect(configuredModelRouteOptions()).toEqual([
      { routeId: "runtime.llm.deep", label: "Deep Research" },
      { routeId: "runtime.llm", label: "Default Radar" }
    ]);
  });

  it("posts a research scan to the Agent run endpoint", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 77, traceId: "agent-77", status: "succeeded" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_API_BASE_URL", "http://localhost:62601");

    await expect(
      createResearchRadarRun({
        topic: "multimodal RAG",
        filters: ["papers", "datasets"],
        ranking: "importance",
        routeId: "runtime.llm"
      })
    ).resolves.toMatchObject({ runId: 77, traceId: "agent-77" });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/agents/runs",
      expect.objectContaining({
        method: "POST",
        body: expect.stringContaining('"webSearchEnabled":true')
      })
    );
  });
});
