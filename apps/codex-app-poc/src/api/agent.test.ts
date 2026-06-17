import { afterEach, describe, expect, it, vi } from "vitest";
import { createAgentRun, createConfiguredModelAgentRun, fetchAgentRunEventStream } from "./agent";

describe("codex poc agent api", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("sends model loop runtime mode", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 1, status: "succeeded", traceId: "agent-1" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await createAgentRun({ input: "search policy", runtimeMode: "model_loop" });

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ input: "search policy", runtimeMode: "model_loop" })
      })
    );
  });

  it("sends configured model route id when the POC env selects one", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 2, status: "queued", traceId: "agent-2" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", " runtime.llm.code_agent ");

    await createConfiguredModelAgentRun("search policy");

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          input: "search policy",
          runtimeMode: "model_loop",
          autoApprove: false,
          modelRouteId: "runtime.llm.code_agent",
          budget: {
            maxSteps: 8,
            maxToolCalls: 1,
            maxSeconds: 60,
            maxCostCents: 0
          }
        })
      })
    );
  });

  it("omits configured model route id when the POC env is blank", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 3, status: "succeeded", traceId: "agent-3" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", "   ");

    await createConfiguredModelAgentRun("search policy");

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          input: "search policy",
          runtimeMode: "model_loop",
          autoApprove: false,
          budget: {
            maxSteps: 8,
            maxToolCalls: 1,
            maxSeconds: 60,
            maxCostCents: 0
          }
        })
      })
    );
  });

  it("opens an agent event stream with cursor query", async () => {
    const fetchMock = vi.fn(async () => new Response("", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await fetchAgentRunEventStream(7, {
      afterSequenceNo: 4,
      batchSize: 10
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:4398/ai/agents/runs/7/events/stream?afterSequenceNo=4&batchSize=10",
      expect.objectContaining({
        method: "GET",
        headers: expect.objectContaining({
          Accept: "text/event-stream"
        })
      })
    );
  });
});
