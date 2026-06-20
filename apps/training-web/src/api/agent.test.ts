import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createAgentRun } from "./agent";

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

describe("training agent api", () => {
  const fetchMock = vi.fn<typeof fetch>(() =>
    okResponse({
      runId: 900,
      traceId: "agent-900",
      status: "succeeded",
      intent: "training_quiz",
      loopKind: "react",
      selectedToolCode: null,
      pauseReason: null,
      finalOutput: "测验已生成：请根据培训资料回答 5 道题。",
      taskBudget: {
        maxSteps: 6,
        maxToolCalls: 0,
        maxSeconds: 30,
        maxCostCents: 0
      },
      createTime: "2026-06-05 12:00:00",
      updateTime: "2026-06-05 12:00:01"
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

  it("creates a bounded training quiz skill run", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await createAgentRun({
      input: "为信息安全入职培训生成 5 道测验题",
      autoApprove: true,
      budget: {
        maxSteps: 6,
        maxToolCalls: 0,
        maxSeconds: 30,
        maxCostCents: 0
      }
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:62601/ai/agents/runs");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1",
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        input: "为信息安全入职培训生成 5 道测验题",
        autoApprove: true,
        budget: {
          maxSteps: 6,
          maxToolCalls: 0,
          maxSeconds: 30,
          maxCostCents: 0
        }
      })
    });
  });
});
