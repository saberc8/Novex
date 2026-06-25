import { afterEach, describe, expect, it, vi } from "vitest";
import { apiRequest } from "./api";

describe("research radar api client", () => {
  afterEach(() => {
    window.localStorage.clear();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it("explains which backend URL failed when fetch cannot connect", async () => {
    vi.stubEnv("NEXT_PUBLIC_API_BASE_URL", "http://localhost:62601");
    vi.stubGlobal("fetch", vi.fn(async () => {
      throw new TypeError("Failed to fetch");
    }));

    await expect(apiRequest("/ai/agents/runs", { method: "POST" })).rejects.toThrow(
      "无法连接 Novex Backend：http://localhost:62601"
    );
  });

  it("logs in with local dev credentials and retries when the backend rejects an anonymous request", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => ({ code: "401", msg: "未授权，请重新登录" })
      })
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => ({
          code: "200",
          data: { token: "radar-dev-token", expire: "2099-01-01T00:00:00Z" }
        })
      })
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => ({ code: "200", data: { runId: 7, traceId: "agent-7", status: "succeeded" } })
      });
    vi.stubGlobal("fetch", fetchMock);

    const result = await apiRequest<{ runId: number }>("/ai/agents/runs", {
      method: "POST",
      body: JSON.stringify({ input: "hello" })
    });

    expect(result.runId).toBe(7);
    expect(window.localStorage.getItem("novex_token")).toBe("radar-dev-token");
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "http://localhost:62601/auth/login",
      expect.objectContaining({
        body: JSON.stringify({
          username: "admin",
          password: "admin123",
          authType: "ACCOUNT",
          clientId: "research-radar-poc"
        }),
        method: "POST"
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "http://localhost:62601/ai/agents/runs",
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: "Bearer radar-dev-token"
        })
      })
    );
  });
});
