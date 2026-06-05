import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { listEvalDatasets, listEvalResults, listEvalRuns, runEval } from "./eval";

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

describe("training eval api", () => {
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

  it("lists eval datasets by code with bearer auth", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await listEvalDatasets({ page: 1, size: 20, code: "training_regression" });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/evals/datasets?page=1&size=20&code=training_regression"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1"
      })
    });
  });

  it("runs a regression eval set by dataset code", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockImplementationOnce(() =>
      okResponse({
        runId: 800,
        datasetId: 700,
        datasetCode: "training_regression",
        status: "succeeded",
        totalCases: 3,
        passedCases: 2,
        failedCases: 1,
        averageScore: 0.67,
        metricBreakdown: {},
        reportPayload: {},
        createTime: "2026-06-05 12:00:00",
        finishedAt: "2026-06-05 12:00:01"
      })
    );

    await runEval({ datasetCode: "training_regression" });

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:4398/ai/evals/runs");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1",
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        datasetCode: "training_regression"
      })
    });
  });

  it("lists recent regression runs by dataset code", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await listEvalRuns({ page: 1, size: 5, datasetCode: "training_regression" });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/evals/runs?page=1&size=5&datasetCode=training_regression"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1"
      })
    });
  });

  it("lists eval case results for a run", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await listEvalResults(800, { page: 1, size: 5 });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/evals/runs/800/results?page=1&size=5"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1"
      })
    });
  });
});
