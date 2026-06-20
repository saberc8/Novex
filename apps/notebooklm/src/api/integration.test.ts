import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resolvePublicShare } from "./integration";

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

describe("chat integration api", () => {
  const fetchMock = vi.fn<typeof fetch>(() =>
    okResponse({
      accepted: true,
      targetPath: "/chat",
      auth: {
        principalType: "publicLink",
        tenantId: 1,
        appId: "llm_chat",
        name: "Published Chat",
        path: "/chat",
        maskedCredential: "nxl_****1234",
        permissionScope: ["app:chat:use"],
        qpsLimit: 2,
        quotaLimit: 200,
        expiresAt: "2026-12-31 00:00:00",
        lastUsedAt: null
      }
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

  it("resolves public links without sending bearer auth", async () => {
    window.localStorage.setItem("novex_token", "private-token");

    await resolvePublicShare("nxl_public_token_1234");

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:62601/share/nxl_public_token_1234");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET"
    });
    expect((fetchMock.mock.calls[0]?.[1] as RequestInit | undefined)?.headers).not.toMatchObject({
      Authorization: expect.any(String)
    });
  });
});
