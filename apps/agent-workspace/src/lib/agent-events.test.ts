import { describe, expect, it } from "vitest";
import { summarizeModelDeltas } from "@/lib/agent-events";
import type { AgentRunEventResp } from "@/types/agent";

function event(overrides: Partial<AgentRunEventResp> = {}): AgentRunEventResp {
  return {
    id: 100,
    runId: 42,
    stepId: null,
    eventType: "thought",
    sequenceNo: 3,
    status: "running",
    payload: {
      item: {
        type: "model_delta",
        routeId: "runtime.llm.code_agent",
        provider: "openai-compatible",
        model: "gpt-compatible",
        deltaIndex: 0,
        content: "Hello"
      }
    },
    createTime: "2026-06-17 12:00:00",
    ...overrides
  };
}

describe("agent event presentation", () => {
  it("summarizes model delta chunks in delta order without trimming token whitespace", () => {
    const summary = summarizeModelDeltas([
      event({
        id: 101,
        sequenceNo: 1,
        eventType: "tool_called",
        payload: {
          item: {
            type: "tool_call",
            content: "ignored"
          }
        }
      }),
      event({
        id: 102,
        sequenceNo: 3,
        payload: {
          item: {
            type: "model_delta",
            routeId: "runtime.llm.code_agent",
            provider: "openai-compatible",
            model: "gpt-compatible",
            deltaIndex: 1,
            content: " world"
          }
        }
      }),
      event({
        id: 103,
        sequenceNo: 2,
        payload: {
          item: {
            type: "model_delta",
            routeId: "runtime.llm.code_agent",
            provider: "openai-compatible",
            model: "gpt-compatible",
            deltaIndex: 0,
            content: "Hello"
          }
        }
      })
    ]);

    expect(summary?.text).toBe("Hello world");
    expect(summary?.chunkCount).toBe(2);
    expect(summary?.routeId).toBe("runtime.llm.code_agent");
    expect(summary?.provider).toBe("openai-compatible");
    expect(summary?.model).toBe("gpt-compatible");
  });
});
