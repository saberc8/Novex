import { afterEach, describe, expect, it, vi } from "vitest";
import { listMcpServers, listMcpTools, listSkills } from "./capability";

describe("capability api", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("lists skills", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: { list: [], total: 0 } })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await listSkills({ page: 1, size: 20 });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/capabilities/skills?page=1&size=20",
      expect.objectContaining({ method: "GET" })
    );
  });

  it("lists MCP servers and server tools", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: { list: [], total: 0 } })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await listMcpServers({ page: 1, size: 20 });
    await listMcpTools(12);

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:62601/ai/capabilities/mcp/servers?page=1&size=20",
      expect.objectContaining({ method: "GET" })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "http://localhost:62601/ai/capabilities/mcp/servers/12/tools",
      expect.objectContaining({ method: "GET" })
    );
  });
});
