import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cancelAgentRun,
  createAgentRun,
  fetchAgentRunEventStream,
  listAgentRunEvents,
  listAgentRuns,
  resumeAgentRun
} from "./agent";

function okResponse(data: unknown) {
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

describe("agent workspace api", () => {
  const fetchMock = vi.fn<typeof fetch>(() =>
    okResponse({
      list: [],
      total: 0
    })
  );

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("creates an agent run with task budget and bearer auth", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockImplementationOnce(() =>
      okResponse({
        runId: 42,
        traceId: "trace-42",
        status: "waiting_approval",
        intent: "tool_task",
        loopKind: "react",
        selectedToolCode: "feishu.message.send",
        pauseReason: "approval",
        finalOutput: null,
        taskBudget: { maxSteps: 6, maxToolCalls: 1 },
        createTime: "2026-06-05 12:00:00",
        updateTime: null
      })
    );

    await createAgentRun({
      input: "send Feishu training reminder",
      runtimeMode: "model_loop",
      autoApprove: false,
      budget: { maxSteps: 6, maxToolCalls: 1 }
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:4398/ai/agents/runs");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1",
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        input: "send Feishu training reminder",
        runtimeMode: "model_loop",
        autoApprove: false,
        budget: { maxSteps: 6, maxToolCalls: 1 }
      })
    });
  });

  it("uses run list, event snapshot, resume, and cancel endpoints", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    await listAgentRuns({ page: 1, size: 20, status: "waiting_approval" });
    await listAgentRunEvents(42, { page: 1, size: 100 });
    await resumeAgentRun(42, {
      approved: true,
      input: { source: "agent-workspace" }
    });
    await cancelAgentRun(42);

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/agents/runs?page=1&size=20&status=waiting_approval"
    );
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:4398/ai/agents/runs/42/events?page=1&size=100"
    );
    expect(fetchMock.mock.calls[2]?.[0]).toBe("http://localhost:4398/ai/agents/runs/42/resume");
    expect(fetchMock.mock.calls[2]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        approved: true,
        input: { source: "agent-workspace" }
      })
    });
    expect(fetchMock.mock.calls[3]?.[0]).toBe("http://localhost:4398/ai/agents/runs/42/cancel");
    expect(fetchMock.mock.calls[3]?.[1]).toMatchObject({
      method: "POST"
    });
  });

  it("opens the agent run event stream with bearer auth and cursor query", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockResolvedValueOnce(new Response("", { status: 200 }));

    await fetchAgentRunEventStream(42, {
      afterSequenceNo: 9,
      batchSize: 25,
      pollMs: 500,
      maxIdleMs: 30000
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/agents/runs/42/events/stream?afterSequenceNo=9&batchSize=25&pollMs=500&maxIdleMs=30000"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET",
      headers: expect.objectContaining({
        Accept: "text/event-stream",
        Authorization: "Bearer token-1"
      })
    });
  });
});
