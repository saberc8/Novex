import { afterEach, describe, expect, it, vi } from "vitest";
import { createAgentRun, fetchAgentRunEventStream } from "./agent";

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
