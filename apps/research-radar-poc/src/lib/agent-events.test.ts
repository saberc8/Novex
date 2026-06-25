import { describe, expect, it } from "vitest";
import { summarizeModelDeltas, summarizeResearchEvent } from "./agent-events";
import type { AgentRunEventResp } from "@/types/agent";

describe("agent event summaries", () => {
  it("orders model delta chunks by delta index", () => {
    const events: AgentRunEventResp[] = [
      {
        id: 2,
        runId: 7,
        eventType: "thought",
        sequenceNo: 2,
        status: "running",
        payload: { item: { type: "model_delta", deltaIndex: 1, content: " world" } },
        createTime: ""
      },
      {
        id: 1,
        runId: 7,
        eventType: "thought",
        sequenceNo: 1,
        status: "running",
        payload: { item: { type: "model_delta", deltaIndex: 0, content: "Hello" } },
        createTime: ""
      }
    ];

    expect(summarizeModelDeltas(events)).toEqual({
      text: "Hello world",
      chunkCount: 2,
      routeId: undefined,
      model: undefined
    });
  });

  it("summarizes dry-run tool observations clearly", () => {
    const event: AgentRunEventResp = {
      id: 3,
      runId: 7,
      eventType: "thought",
      sequenceNo: 3,
      status: "succeeded",
      payload: {
        item: {
          type: "tool_observation",
          toolCode: "web.search",
          output: { dryRun: true, status: "dry_run" }
        }
      },
      createTime: ""
    };

    expect(summarizeResearchEvent(event)).toEqual({
      sequenceNo: 3,
      title: "Web search",
      kind: "tool",
      text: "dry-run: web.search returned no live provider result"
    });
  });
});
