import { describe, expect, it, vi } from "vitest";
import {
  agentRunPayload,
  assertAgentSmokeEvidence,
  runAgentLiveSmoke,
  smokeConfigFromEnv
} from "./agent-live-smoke.mjs";

function okEnvelope(data) {
  return {
    ok: true,
    status: 200,
    json: async () => ({
      code: "200",
      data
    })
  };
}

describe("agent live smoke runner", () => {
  it("skips without explicit live smoke flag", async () => {
    const fetchMock = vi.fn();
    const logger = { log: vi.fn(), error: vi.fn() };

    const result = await runAgentLiveSmoke({
      env: {},
      fetch: fetchMock,
      logger
    });

    expect(result).toEqual({ skipped: true });
    expect(fetchMock).not.toHaveBeenCalled();
    expect(logger.log).toHaveBeenCalledWith(
      expect.stringContaining("NOVEX_LIVE_AGENT_SMOKE=1")
    );
  });

  it("builds a configured model-loop payload with a trimmed route id", () => {
    const config = smokeConfigFromEnv({
      NOVEX_LIVE_AGENT_SMOKE: "1",
      NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID: " runtime.llm.code_agent ",
      NOVEX_AGENT_SMOKE_INPUT: " prove the loop "
    });

    expect(agentRunPayload(config)).toEqual({
      input: "prove the loop",
      runtimeMode: "model_loop",
      autoApprove: false,
      modelRouteId: "runtime.llm.code_agent",
      budget: {
        maxSteps: 8,
        maxToolCalls: 1,
        maxSeconds: 60,
        maxCostCents: 0
      }
    });
  });

  it("creates a run, polls events, and returns model inference evidence", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(okEnvelope({ runId: 42, status: "running", traceId: "agent-42" }))
      .mockResolvedValueOnce(
        okEnvelope({
          list: [
            {
              runId: 42,
              status: "succeeded",
              eventType: "thought",
              sequenceNo: 1,
              payload: {
                runtimeMode: "model_loop",
                item: {
                  type: "model_inference",
                  routeId: "runtime.llm.code_agent",
                  provider: "deep-seek"
                }
              }
            }
          ],
          total: 1
        })
      );
    const logger = { log: vi.fn(), error: vi.fn() };

    const result = await runAgentLiveSmoke({
      env: {
        NOVEX_LIVE_AGENT_SMOKE: "1",
        NEXT_PUBLIC_API_BASE_URL: "http://localhost:4398",
        NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID: "runtime.llm.code_agent",
        NOVEX_AGENT_SMOKE_TOKEN: "jwt-1",
        NOVEX_AGENT_SMOKE_MAX_POLLS: "1"
      },
      fetch: fetchMock,
      logger
    });

    expect(result).toMatchObject({
      skipped: false,
      runId: 42,
      status: "succeeded",
      routeId: "runtime.llm.code_agent"
    });
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:4398/ai/agents/runs",
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          Authorization: "Bearer jwt-1",
          "Content-Type": "application/json"
        })
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "http://localhost:4398/ai/agents/runs/42/events?page=1&size=100",
      expect.objectContaining({
        method: "GET",
        headers: expect.objectContaining({
          Authorization: "Bearer jwt-1"
        })
      })
    );
  });

  it("fails when requested route id does not match model inference evidence", () => {
    const config = smokeConfigFromEnv({
      NOVEX_LIVE_AGENT_SMOKE: "1",
      NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID: "runtime.llm.code_agent"
    });

    expect(() =>
      assertAgentSmokeEvidence(
        config,
        { runId: 42, status: "succeeded" },
        [
          {
            status: "succeeded",
            payload: {
              item: {
                type: "model_inference",
                routeId: "runtime.llm.backup"
              }
            }
          }
        ]
      )
    ).toThrow("expected modelRouteId runtime.llm.code_agent");
  });

  it("fails when polling reaches the configured attempt limit", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(okEnvelope({ runId: 42, status: "running", traceId: "agent-42" }))
      .mockResolvedValue(okEnvelope({ list: [], total: 0 }));

    await expect(
      runAgentLiveSmoke({
        env: {
          NOVEX_LIVE_AGENT_SMOKE: "1",
          NOVEX_AGENT_SMOKE_MAX_POLLS: "2",
          NOVEX_AGENT_SMOKE_POLL_MS: "0"
        },
        fetch: fetchMock,
        logger: { log: vi.fn(), error: vi.fn() }
      })
    ).rejects.toThrow("timed out waiting for Agent run 42");
  });
});
