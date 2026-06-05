import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { askDataset, listDatasets, submitRagFeedback, uploadKnowledgeFile } from "./knowledge";
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

  it("uploads knowledge files as multipart form data with bearer auth", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockImplementationOnce(() =>
      okResponse({
        file: {
          id: 88,
          name: "88.md",
          originalName: "handbook.md",
          size: 24,
          url: "/file/knowledge/88.md",
          parentPath: "/knowledge",
          path: "/knowledge/88.md",
          sha256: "hash",
          contentType: "text/markdown",
          metadata: "{}",
          thumbnailSize: 0,
          thumbnailName: "",
          thumbnailMetadata: "",
          thumbnailUrl: "",
          extension: "md",
          type: 4,
          storageId: 1,
          storageName: "本地",
          createUserString: "admin",
          createTime: "2026-06-05 10:00:00",
          updateUserString: "",
          updateTime: ""
        },
        parseJob: {
          id: 99,
          tenantId: 1,
          datasetId: 10,
          documentId: 42,
          jobType: 2,
          status: 2,
          attemptCount: 0,
          errorMessage: "",
          resultSummary: {},
          documentName: "handbook.md",
          sourceUri: "/file/knowledge/88.md",
          fileId: 88,
          contentType: "text/markdown",
          parseStatus: 2,
          ingestionStatus: 1,
          chunkCount: 0,
          parserRequest: {},
          createUserString: "",
          createTime: "2026-06-05 10:00:00",
          updateUserString: "",
          updateTime: ""
        }
      })
    );
    const file = new File(["# Handbook"], "handbook.md", { type: "text/markdown" });

    await uploadKnowledgeFile(10, file);

    const [, init] = fetchMock.mock.calls[0] ?? [];
    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/knowledge/datasets/10/documents/files"
    );
    expect(init).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1"
      })
    });
    expect((init?.headers as Record<string, string>)["Content-Type"]).toBeUndefined();
    expect(init?.body).toBeInstanceOf(FormData);
    const uploadedFile = (init?.body as FormData).get("file") as File;
    expect(uploadedFile.name).toBe("handbook.md");
    expect(uploadedFile.type).toBe("text/markdown");
    expect((init?.body as FormData).get("parentPath")).toBe("/knowledge");
  });

  it("reads auth token safely when browser storage is unavailable", () => {
    vi.stubGlobal("window", undefined);

    expect(getAuthToken()).toBeNull();
  });
});
