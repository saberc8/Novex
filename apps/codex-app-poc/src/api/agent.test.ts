import { afterEach, describe, expect, it, vi } from "vitest";
import { createAgentRun } from "./agent";

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
});
