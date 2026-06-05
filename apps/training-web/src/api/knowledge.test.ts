import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { askDataset, listDatasets, submitRagFeedback } from "./knowledge";
import { getAuthToken } from "@/lib/auth";

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

describe("training knowledge api", () => {
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

  it("lists datasets through the backend knowledge endpoint", async () => {
    await listDatasets({ page: 2, size: 10 });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/knowledge/datasets?page=2&size=10"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET"
    });
  });

  it("sends ask requests with bearer auth when a token exists", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockImplementationOnce(() =>
      okResponse({
        traceId: 42,
        answer: "Training starts on Monday.",
        citations: [],
        retrievalHitCount: 1,
        answerStrategy: "extractive"
      })
    );

    await askDataset(10, {
      question: "培训什么时候开始？",
      limit: 5
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/knowledge/datasets/10/ask"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1",
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        question: "培训什么时候开始？",
        limit: 5
      })
    });
  });

  it("submits RAG feedback with trace id and rating", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockImplementationOnce(() =>
      okResponse({
        id: 99,
        traceId: 42,
        rating: "not_helpful"
      })
    );

    await submitRagFeedback({
      traceId: 42,
      rating: "not_helpful",
      reason: "答案没有覆盖培训截止时间"
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:4398/ai/knowledge/feedback");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1",
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        traceId: 42,
        rating: "not_helpful",
        reason: "答案没有覆盖培训截止时间"
      })
    });
  });

  it("reads auth token safely when browser storage is unavailable", () => {
    vi.stubGlobal("window", undefined);

    expect(getAuthToken()).toBeNull();
  });
});
