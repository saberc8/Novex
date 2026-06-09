import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { chatCompletion, getModelRuntimeConfig, listChatConversations } from "./model";

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

describe("chat model api", () => {
  const fetchMock = vi.fn<typeof fetch>(() =>
    okResponse({
      answer: "Pure model answer.",
      routeId: "runtime.llm",
      model: "deepseek-v4-flash",
      latencyMs: 42,
      usage: {
        promptTokens: 11,
        completionTokens: 7,
        totalTokens: 18
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

  it("sends pure model chat messages with bearer auth", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await chatCompletion({
      messages: [{ role: "user", content: "Explain Novex." }],
      fileContexts: [
        {
          name: "handbook.md",
          contentType: "text/markdown",
          content: "# Handbook"
        }
      ],
      temperature: 0.2,
      maxTokens: 1024
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:4398/ai/models/chat");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1",
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        messages: [{ role: "user", content: "Explain Novex." }],
        fileContexts: [
          {
            name: "handbook.md",
            contentType: "text/markdown",
            content: "# Handbook"
          }
        ],
        temperature: 0.2,
        maxTokens: 1024
      })
    });
  });

  it("loads model chat conversations with bearer auth", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await listChatConversations();

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:4398/ai/models/chat/conversations");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1"
      })
    });
  });

  it("loads runtime config with bearer auth", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await getModelRuntimeConfig();

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:4398/ai/models/runtime-config");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1"
      })
    });
  });
});
