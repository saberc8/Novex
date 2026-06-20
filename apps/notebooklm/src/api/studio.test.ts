import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  deleteStudioArtifact,
  generateStudioArtifact,
  listDatasetStudioArtifacts,
  listStudioActions
} from "./studio";

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

describe("chat studio api", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse([]));

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("lists knowledge studio actions", async () => {
    await listStudioActions({ surface: "knowledge" });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:62601/ai/studio/actions?surface=knowledge"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET"
    });
  });

  it("lists artifacts for the selected notebook", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await listDatasetStudioArtifacts(10);

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:62601/ai/knowledge/datasets/10/artifacts"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1"
      })
    });
  });

  it("generates a cited mind map artifact with the selected model route", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await generateStudioArtifact(10, {
      actionCode: "mind_map.generate",
      topic: "Training handbook",
      maxNodes: 10,
      answerModelRouteId: "runtime.llm.rag_answer"
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:62601/ai/knowledge/datasets/10/artifacts"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1",
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        actionCode: "mind_map.generate",
        topic: "Training handbook",
        maxNodes: 10,
        answerModelRouteId: "runtime.llm.rag_answer"
      })
    });
  });

  it("deletes a generated studio artifact", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await deleteStudioArtifact(8801);

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:62601/ai/studio/artifacts/8801"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "DELETE",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1"
      })
    });
  });
});
