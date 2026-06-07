import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { listExternalAccounts, listIdentityPolicies, listIdentityProviders } from "@/api/system/identity";

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

describe("system identity api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses identity provider, account, and policy endpoints", async () => {
    await listIdentityProviders({ page: 1, size: 20, providerType: "github" });
    await listExternalAccounts({ providerCode: "github.login" });
    await listIdentityPolicies({ page: 1, size: 20 });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/system/identity/providers?page=1&size=20&providerType=github"
    );
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:4398/system/identity/accounts?providerCode=github.login"
    );
    expect(fetchMock.mock.calls[2]?.[0]).toBe(
      "http://localhost:4398/system/identity/policies?page=1&size=20"
    );
  });
});
