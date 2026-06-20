import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  getEvalRun,
  listEvalCases,
  listEvalDatasets,
  listEvalResults,
  listEvalRuns,
  runEvalDataset
} from "@/api/ai/eval";

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

describe("eval api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses eval dataset, case, run, report, and result endpoints", async () => {
    await listEvalDatasets({ page: 1, size: 20, code: "training_regression" });
    await listEvalCases(3400001, { page: 1, size: 100, targetKind: "rag" });
    await runEvalDataset({ datasetCode: "training_regression" });
    await listEvalRuns({ page: 1, size: 10, datasetCode: "training_regression" });
    await getEvalRun(42);
    await listEvalResults(42, { page: 1, size: 100 });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:62601/ai/evals/datasets?page=1&size=20&code=training_regression"
    );
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:62601/ai/evals/datasets/3400001/cases?page=1&size=100&targetKind=rag"
    );
    expect(fetchMock.mock.calls[2]?.[0]).toBe("http://localhost:62601/ai/evals/runs");
    expect(fetchMock.mock.calls[2]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ datasetCode: "training_regression" })
    });
    expect(fetchMock.mock.calls[3]?.[0]).toBe(
      "http://localhost:62601/ai/evals/runs?page=1&size=10&datasetCode=training_regression"
    );
    expect(fetchMock.mock.calls[4]?.[0]).toBe("http://localhost:62601/ai/evals/runs/42");
    expect(fetchMock.mock.calls[5]?.[0]).toBe(
      "http://localhost:62601/ai/evals/runs/42/results?page=1&size=100"
    );
  });
});
