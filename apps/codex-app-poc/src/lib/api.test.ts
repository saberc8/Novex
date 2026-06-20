import { afterEach, describe, expect, it, vi } from "vitest";
import { apiRequest } from "./api";

describe("codex poc api client", () => {
  afterEach(() => {
    window.localStorage.clear();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it("logs in with local dev credentials and retries when the backend rejects an anonymous request", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ code: "401", msg: "未授权，请重新登录" })
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          code: "200",
          data: { token: "dev-token", expire: "2099-01-01T00:00:00Z" }
        })
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ code: "200", data: { list: [], total: 0 } })
      });
    vi.stubGlobal("fetch", fetchMock);

    const result = await apiRequest<{ list: unknown[]; total: number }>("/ai/capabilities/skills", {
      method: "GET",
      query: { page: 1, size: 20 }
    });

    expect(result).toEqual({ list: [], total: 0 });
    expect(window.localStorage.getItem("novex_token")).toBe("dev-token");
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "http://localhost:62601/auth/login",
      expect.objectContaining({
        body: JSON.stringify({
          username: "admin",
          password: "admin123",
          authType: "ACCOUNT",
          clientId: "codex-app-poc"
        }),
        method: "POST"
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "http://localhost:62601/ai/capabilities/skills?page=1&size=20",
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: "Bearer dev-token"
        })
      })
    );
  });
});
