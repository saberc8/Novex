import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { deleteMemory, listMemories, upsertMemory } from "@/api/ai/memory";

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

describe("memory api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses memory list, upsert, and delete endpoints", async () => {
    await listMemories({ page: 1, size: 20, scopeType: "user", scopeId: "1" });
    await upsertMemory({
      scopeType: "user",
      scopeId: "1",
      sourceKind: "manual",
      sourceId: "note-7",
      content: "prefers concise updates",
      summary: "concise updates",
      sensitivity: "preference",
      writePolicy: "user_approved",
      ttlDays: 90,
      metadata: { confirmedByUser: true },
      status: 1
    });
    await deleteMemory(99);

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/memories?page=1&size=20&scopeType=user&scopeId=1"
    );
    expect(fetchMock.mock.calls[1]?.[0]).toBe("http://localhost:4398/ai/memories");
    expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        scopeType: "user",
        scopeId: "1",
        sourceKind: "manual",
        sourceId: "note-7",
        content: "prefers concise updates",
        summary: "concise updates",
        sensitivity: "preference",
        writePolicy: "user_approved",
        ttlDays: 90,
        metadata: { confirmedByUser: true },
        status: 1
      })
    });
    expect(fetchMock.mock.calls[2]?.[0]).toBe("http://localhost:4398/ai/memories/99");
    expect(fetchMock.mock.calls[2]?.[1]).toMatchObject({ method: "DELETE" });
  });
});
