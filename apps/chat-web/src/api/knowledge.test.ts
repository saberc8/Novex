import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { askDataset, listDatasets, submitRagFeedback } from "./knowledge";

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

describe("chat knowledge api", () => {
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

  it("lists datasets for the source selector", async () => {
    await listDatasets({ page: 1, size: 20 });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/knowledge/datasets?page=1&size=20"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET"
    });
  });

  it("asks the selected knowledge dataset with bearer auth", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockImplementationOnce(() =>
      okResponse({
        traceId: 42,
        answer: "Use the current handbook.",
        citations: [],
        retrievalHitCount: 1,
        answerStrategy: "extractive"
      })
    );

    await askDataset(10, {
      question: "Which handbook should I use?",
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
        question: "Which handbook should I use?",
        limit: 5
      })
    });
  });

  it("submits answer feedback for eval promotion", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockImplementationOnce(() =>
      okResponse({
        id: 99,
        traceId: 42,
        rating: "citation_issue"
      })
    );

    await submitRagFeedback({
      traceId: 42,
      rating: "citation_issue",
      reason: "chat-answer-feedback"
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
        rating: "citation_issue",
        reason: "chat-answer-feedback"
      })
    });
  });
});
