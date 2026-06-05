import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  dryRunTool,
  getCapabilitySummary,
  listConnectors,
  listPlugins,
  listToolAudits,
  listTools,
  listTriggers
} from "@/api/ai/capability";

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

describe("capability api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses capability summary, registry, dry-run, and audit endpoints", async () => {
    await getCapabilitySummary();
    await listTools({ page: 2, size: 20, kind: "media" });
    await listConnectors({ status: 1 });
    await listPlugins();
    await listTriggers();
    await dryRunTool({ toolCode: "rag.search", input: { query: "hello" } });
    await listToolAudits({ page: 1, size: 5, toolCode: "rag.search" });

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:4398/ai/capabilities/summary");
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/tools?page=2&size=20&kind=media"
    );
    expect(fetchMock.mock.calls[2]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/connectors?status=1"
    );
    expect(fetchMock.mock.calls[3]?.[0]).toBe("http://localhost:4398/ai/capabilities/plugins");
    expect(fetchMock.mock.calls[4]?.[0]).toBe("http://localhost:4398/ai/capabilities/triggers");
    expect(fetchMock.mock.calls[5]?.[0]).toBe("http://localhost:4398/ai/capabilities/tools/dry-run");
    expect(fetchMock.mock.calls[5]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ toolCode: "rag.search", input: { query: "hello" } })
    });
    expect(fetchMock.mock.calls[6]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/tools/audits?page=1&size=5&toolCode=rag.search"
    );
  });
});
