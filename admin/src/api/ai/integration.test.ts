import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  createApiKey,
  createPublicLink,
  listApiKeys,
  listPublicLinks,
  revokeApiKey,
  revokePublicLink
} from "@/api/ai/integration";

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

describe("ai integration api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses api-key and public-link control-plane endpoints", async () => {
    await listApiKeys({ page: 1, size: 20, appId: "training_app" });
    await createApiKey({
      appId: "training_app",
      name: "Training API",
      permissionScope: ["app:training:ask"],
      qpsLimit: 5,
      quotaLimit: 1000,
      expiresAt: "2026-12-31T00:00:00Z"
    });
    await revokeApiKey(123);
    await listPublicLinks({ page: 1, size: 20, appId: "training_app" });
    await createPublicLink({
      appId: "training_app",
      name: "Training Preview",
      path: "/ask",
      permissionScope: ["app:training:ask"],
      qpsLimit: 2,
      quotaLimit: 200,
      expiresAt: "2026-12-31T00:00:00Z"
    });
    await revokePublicLink(456);

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:62601/ai/integrations/api-keys?page=1&size=20&appId=training_app"
    );
    expect(fetchMock.mock.calls[1]?.[0]).toBe("http://localhost:62601/ai/integrations/api-keys");
    expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        appId: "training_app",
        name: "Training API",
        permissionScope: ["app:training:ask"],
        qpsLimit: 5,
        quotaLimit: 1000,
        expiresAt: "2026-12-31T00:00:00Z"
      })
    });
    expect(fetchMock.mock.calls[2]?.[0]).toBe("http://localhost:62601/ai/integrations/api-keys/123/revoke");
    expect(fetchMock.mock.calls[3]?.[0]).toBe(
      "http://localhost:62601/ai/integrations/public-links?page=1&size=20&appId=training_app"
    );
    expect(fetchMock.mock.calls[4]?.[0]).toBe("http://localhost:62601/ai/integrations/public-links");
    expect(fetchMock.mock.calls[5]?.[0]).toBe("http://localhost:62601/ai/integrations/public-links/456/revoke");
  });
});
