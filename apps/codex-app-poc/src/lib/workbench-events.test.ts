import { describe, expect, it } from "vitest";
import { summarizeWorkbenchEvent } from "./workbench-events";
import type { AgentRunEventResp } from "@/types/agent";

function event(eventType: string, payload: AgentRunEventResp["payload"]): AgentRunEventResp {
  return {
    id: 1,
    runId: 7,
    stepId: null,
    eventType,
    sequenceNo: 1,
    status: "running",
    payload,
    createTime: "2026-06-18 12:00:00"
  };
}

describe("workbench event summaries", () => {
  it("summarizes model deltas", () => {
    const summary = summarizeWorkbenchEvent(
      event("thought", {
        item: { type: "model_delta", content: "Hello" }
      })
    );

    expect(summary).toMatchObject({
      kind: "assistant_delta",
      title: "Assistant",
      text: "Hello"
    });
  });

  it("summarizes tool calls", () => {
    const summary = summarizeWorkbenchEvent(
      event("tool_called", {
        toolCode: "rag.search",
        arguments: { query: "refund", datasetId: 7 }
      })
    );

    expect(summary.kind).toBe("tool");
    expect(summary.title).toBe("rag.search");
    expect(summary.text).toContain("datasetId");
  });

  it("summarizes retrieval evidence", () => {
    const summary = summarizeWorkbenchEvent(
      event("thought", {
        item: {
          type: "tool_observation",
          toolCode: "rag.search",
          output: { hits: [{ id: 1 }, { id: 2 }], citations: [{ documentId: "11" }] }
        }
      })
    );

    expect(summary.kind).toBe("retrieval");
    expect(summary.title).toBe("Knowledge search");
    expect(summary.text).toContain("2 hits");
  });

  it("summarizes web search dry-run evidence", () => {
    const summary = summarizeWorkbenchEvent(
      event("thought", {
        item: {
          type: "tool_observation",
          toolCode: "web.search",
          output: { dryRun: true, status: "dry_run", query: "fresh facts", results: [] }
        }
      })
    );

    expect(summary.kind).toBe("web_search");
    expect(summary.title).toBe("Web search");
    expect(summary.text).toContain("dry-run");
  });

  it("summarizes failed web search evidence with the provider error", () => {
    const summary = summarizeWorkbenchEvent(
      event("observation", {
        item: {
          type: "tool_observation",
          toolCode: "web.search",
          output: {
            dryRun: false,
            status: "failed",
            provider: "google_news_rss",
            error: "web.search dispatch failed: error sending request"
          }
        }
      })
    );

    expect(summary.kind).toBe("web_search");
    expect(summary.title).toBe("Web search");
    expect(summary.status).toBe("running");
    expect(summary.text).toContain("google_news_rss");
    expect(summary.text).toContain("error sending request");
  });

  it("keeps raw fallback evidence readable", () => {
    const summary = summarizeWorkbenchEvent(event("unknown", { hello: "world" }));

    expect(summary.kind).toBe("raw");
    expect(summary.title).toBe("unknown");
    expect(summary.raw).toEqual({ hello: "world" });
  });
});
