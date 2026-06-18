import { afterEach, describe, expect, it, vi } from "vitest";
import { createDataset, getParseJob, listDatasets, uploadKnowledgeFile } from "./knowledge";

describe("knowledge api", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("lists datasets by name", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: { list: [], total: 0 } })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await listDatasets({ name: "Codex Workbench Inbox", page: 1, size: 10 });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:4398/ai/knowledge/datasets?name=Codex+Workbench+Inbox&page=1&size=10",
      expect.objectContaining({ method: "GET" })
    );
  });

  it("creates datasets through JSON API", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: 7 })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await createDataset({ name: "Codex Workbench Inbox", visibility: 1, retrievalMode: 1 });

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/knowledge/datasets"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          name: "Codex Workbench Inbox",
          visibility: 1,
          retrievalMode: 1
        })
      })
    );
  });

  it("uploads files as multipart form data", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: {
          file: { id: 19, originalName: "handbook.md" },
          parseJob: { id: 29, documentId: 11, status: 1 }
        }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await uploadKnowledgeFile(
      7,
      new File(["hello"], "handbook.md", { type: "text/markdown" })
    );

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(String(url)).toContain(
      "/ai/knowledge/datasets/7/documents/files"
    );
    expect(init?.method).toBe("POST");
    expect(init?.body).toBeInstanceOf(FormData);
    expect((init?.headers as Record<string, string>)["Content-Type"]).toBeUndefined();
  });

  it("gets parse jobs", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: { id: 29, status: 2 } })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await getParseJob(7, 29);

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:4398/ai/knowledge/datasets/7/parse-jobs/29",
      expect.objectContaining({ method: "GET" })
    );
  });
});
