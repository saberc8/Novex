import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  askDataset,
  createDataset,
  listDatasets,
  listDocuments,
  uploadTextDocument
} from "@/api/ai/knowledge";

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

describe("knowledge api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses dataset list, create, and document list endpoints", async () => {
    await listDatasets({ page: 2, size: 20, name: "handbook", status: 1 });
    await createDataset({
      name: "员工手册",
      description: "制度与培训资料",
      visibility: 1,
      retrievalMode: 3
    });
    await listDocuments(7, { page: 1, size: 10 });
    await uploadTextDocument(7, {
      name: "handbook.txt",
      content: "Training starts on Monday.",
      contentType: "text/plain"
    });
    await askDataset(7, { question: "When does training start?", limit: 3 });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:62601/ai/knowledge/datasets?page=2&size=20&name=handbook&status=1"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: "GET" });
    expect(fetchMock.mock.calls[1]?.[0]).toBe("http://localhost:62601/ai/knowledge/datasets");
    expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        name: "员工手册",
        description: "制度与培训资料",
        visibility: 1,
        retrievalMode: 3
      })
    });
    expect(fetchMock.mock.calls[2]?.[0]).toBe(
      "http://localhost:62601/ai/knowledge/datasets/7/documents?page=1&size=10"
    );
    expect(fetchMock.mock.calls[3]?.[0]).toBe(
      "http://localhost:62601/ai/knowledge/datasets/7/documents/text"
    );
    expect(fetchMock.mock.calls[3]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        name: "handbook.txt",
        content: "Training starts on Monday.",
        contentType: "text/plain"
      })
    });
    expect(fetchMock.mock.calls[4]?.[0]).toBe("http://localhost:62601/ai/knowledge/datasets/7/ask");
    expect(fetchMock.mock.calls[4]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ question: "When does training start?", limit: 3 })
    });
  });
});
