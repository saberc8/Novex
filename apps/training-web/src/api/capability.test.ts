import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { dryRunTool } from "./capability";

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

describe("training capability api", () => {
  const fetchMock = vi.fn<typeof fetch>(() =>
    okResponse({
      auditId: 901,
      toolCode: "feishu.message.send",
      status: "succeeded",
      dryRun: true,
      response: {
        message: "dry-run only; no external side effects"
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

  it("dry-runs a Feishu reminder without exposing connector governance fields", async () => {
    window.localStorage.setItem("novex_token", "token-1");

    await dryRunTool({
      toolCode: "feishu.message.send",
      input: {
        recipient: "training-team",
        text: "请完成信息安全入职培训"
      }
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:62601/ai/capabilities/tools/dry-run"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        Authorization: "Bearer token-1",
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        toolCode: "feishu.message.send",
        input: {
          recipient: "training-team",
          text: "请完成信息安全入职培训"
        }
      })
    });
  });
});
