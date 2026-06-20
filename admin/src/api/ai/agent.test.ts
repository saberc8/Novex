import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cancelAgentRun,
  createAgentRun,
  getAgentRun,
  listAgentRunEvents,
  listAgentRuns,
  resumeAgentRun
} from "@/api/ai/agent";

function okResponse(data: unknown = true) {
  return Promise.resolve(
    new Response(
      JSON.stringify({
        code: "200",
        data,
        msg: "成功",
        success: true,
        timestamp: "1"
      }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" }
      }
    )
  );
}

describe("agent api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses agent run, event snapshot, resume, and cancel endpoints", async () => {
    await createAgentRun({
      input: "send Feishu training reminder",
      autoApprove: false,
      budget: { maxSteps: 6, maxToolCalls: 2, maxSeconds: 30, maxCostCents: 0 }
    });
    await listAgentRuns({ page: 1, size: 20, status: "waiting_approval" });
    await getAgentRun(42);
    await listAgentRunEvents(42, { page: 1, size: 100 });
    await resumeAgentRun(42, { approved: true, input: { note: "approved" } });
    await cancelAgentRun(42);

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:62601/ai/agents/runs");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        input: "send Feishu training reminder",
        autoApprove: false,
        budget: { maxSteps: 6, maxToolCalls: 2, maxSeconds: 30, maxCostCents: 0 }
      })
    });
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:62601/ai/agents/runs?page=1&size=20&status=waiting_approval"
    );
    expect(fetchMock.mock.calls[2]?.[0]).toBe("http://localhost:62601/ai/agents/runs/42");
    expect(fetchMock.mock.calls[3]?.[0]).toBe(
      "http://localhost:62601/ai/agents/runs/42/events?page=1&size=100"
    );
    expect(fetchMock.mock.calls[4]?.[0]).toBe("http://localhost:62601/ai/agents/runs/42/resume");
    expect(fetchMock.mock.calls[4]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ approved: true, input: { note: "approved" } })
    });
    expect(fetchMock.mock.calls[5]?.[0]).toBe("http://localhost:62601/ai/agents/runs/42/cancel");
    expect(fetchMock.mock.calls[5]?.[1]).toMatchObject({ method: "POST" });
  });
});
