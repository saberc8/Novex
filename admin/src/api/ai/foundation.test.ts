import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getFoundationSummary } from "@/api/ai/foundation";

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

describe("foundation api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses the foundation summary endpoint", async () => {
    await getFoundationSummary();

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:62601/ai/foundation/summary");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET"
    });
  });
});
