import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  deleteModelRegistryRoute,
  getModelRegistry,
  getModelRuntimeConfig,
  runModelHealthCheck,
  upsertModelRegistryRoute
} from "@/api/ai/model";

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

describe("model runtime api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses runtime config, registry, upsert, and health-check endpoints", async () => {
    await getModelRuntimeConfig();
    await getModelRegistry();
    await upsertModelRegistryRoute({
      providerCode: "deepseek",
      providerName: "DeepSeek",
      providerType: "deep-seek",
      deploymentCode: "deepseek-public",
      deploymentName: "DeepSeek Public API",
      endpoint: "https://api.deepseek.com",
      apiPath: "/chat/completions",
      profileCode: "deepseek-v4-flash",
      profileName: "DeepSeek V4 Flash",
      modelName: "deepseek-v4-flash",
      modelKind: "llm",
      credentialCode: "env-llm-api-key",
      credentialRef: "env:LLM_API_KEY",
      routeCode: "runtime.llm.chat",
      routePurpose: "chat",
      priority: 100
    });
    await deleteModelRegistryRoute(42);
    await runModelHealthCheck({ target: "all" });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/models/runtime-config"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: "GET" });
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:4398/ai/models/registry"
    );
    expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({ method: "GET" });
    expect(fetchMock.mock.calls[2]?.[0]).toBe(
      "http://localhost:4398/ai/models/registry/routes"
    );
    expect(fetchMock.mock.calls[2]?.[1]).toMatchObject({
      method: "POST",
      body: expect.stringContaining('"credentialRef":"env:LLM_API_KEY"')
    });
    expect(fetchMock.mock.calls[3]?.[0]).toBe(
      "http://localhost:4398/ai/models/registry/routes/42"
    );
    expect(fetchMock.mock.calls[3]?.[1]).toMatchObject({ method: "DELETE" });
    expect(fetchMock.mock.calls[4]?.[0]).toBe(
      "http://localhost:4398/ai/models/health-check"
    );
    expect(fetchMock.mock.calls[4]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ target: "all" })
    });
  });
});
