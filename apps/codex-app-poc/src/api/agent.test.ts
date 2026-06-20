import { afterEach, describe, expect, it, vi } from "vitest";
import {
  agentRunEventWebSocketUrl,
  createAgentRun,
  createAgentRunEventWebSocketTicket,
  createConfiguredModelAgentRun,
  fetchAgentRunEventStream,
  listAgentRunEvents
} from "./agent";

describe("codex poc agent api", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    window.localStorage.clear();
  });

  it("sends model loop runtime mode", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 1, status: "succeeded", traceId: "agent-1" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await createAgentRun({ input: "search policy", runtimeMode: "model_loop" });

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ input: "search policy", runtimeMode: "model_loop" })
      })
    );
  });

  it("sends configured model route id when the POC env selects one", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 2, status: "queued", traceId: "agent-2" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", " runtime.llm.code_agent ");

    await createConfiguredModelAgentRun("search policy");

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          input: "search policy",
          runtimeMode: "model_loop",
          autoApprove: false,
          modelRouteId: "runtime.llm.code_agent",
          budget: {
            maxSteps: 8,
            maxToolCalls: 2,
            maxSeconds: 90,
            maxCostCents: 0
          },
          workbenchContext: {
            mode: "agent",
            documentIds: [],
            fileIds: [],
            skillCodes: [],
            mcpToolCodes: [],
            webSearchEnabled: false,
            routeId: "runtime.llm.code_agent"
          }
        })
      })
    );
  });

  it("omits configured model route id when the POC env is blank", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 3, status: "succeeded", traceId: "agent-3" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", "   ");

    await createConfiguredModelAgentRun("search policy");

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          input: "search policy",
          runtimeMode: "model_loop",
          autoApprove: false,
          budget: {
            maxSteps: 8,
            maxToolCalls: 2,
            maxSeconds: 90,
            maxCostCents: 0
          },
          workbenchContext: {
            mode: "agent",
            documentIds: [],
            fileIds: [],
            skillCodes: [],
            mcpToolCodes: [],
            webSearchEnabled: false
          }
        })
      })
    );
  });

  it("sends workbench context with configured model run", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 4, status: "queued", traceId: "agent-4" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await createConfiguredModelAgentRun("Summarize file", {
      mode: "agent",
      datasetId: 7,
      documentIds: [11],
      fileIds: [19],
      skillCodes: ["support.refund"],
      mcpToolCodes: ["mcp.docs.search"],
      webSearchEnabled: true
    });

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
        method: "POST",
        body: expect.stringContaining('"workbenchContext"')
      })
    );
    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(String(init.body)).toContain('"skillCodes":["support.refund"]');
    expect(String(init.body)).toContain('"webSearchEnabled":true');
  });

  it("opens an agent event stream with cursor query", async () => {
    const fetchMock = vi.fn(async () => new Response("", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await fetchAgentRunEventStream(7, {
      afterSequenceNo: 4,
      batchSize: 10
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/agents/runs/7/events/stream?afterSequenceNo=4&batchSize=10",
      expect.objectContaining({
        method: "GET",
        headers: expect.objectContaining({
          Accept: "text/event-stream"
        })
      })
    );
  });

  it("lists agent run events for a POC run snapshot", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: {
          list: [
            {
              id: 11,
              runId: 7,
              eventType: "thought",
              sequenceNo: 5,
              status: "running",
              payload: {
                item: {
                  type: "model_delta",
                  content: "Hello"
                }
              },
              createTime: "2026-06-17 12:00:00"
            }
          ],
          total: 1
        }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);

    const page = await listAgentRunEvents(7, { page: 1, size: 100 });

    expect(page.total).toBe(1);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/agents/runs/7/events?page=1&size=100",
      expect.objectContaining({
        method: "GET"
      })
    );
  });

  it("requests an agent event websocket ticket with bearer auth", async () => {
    window.localStorage.setItem("novex_token", "token-1");
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: {
          ticket: "ws-ticket-1",
          expiresInSeconds: 60
        }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);

    const ticket = await createAgentRunEventWebSocketTicket(7);

    expect(ticket).toEqual({
      ticket: "ws-ticket-1",
      expiresInSeconds: 60
    });
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/agents/runs/7/events/ws-ticket",
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          Authorization: "Bearer token-1"
        })
      })
    );
  });

  it("builds an agent event websocket url with cursor query and ticket", () => {
    const url = agentRunEventWebSocketUrl(7, "ws-ticket-1", {
      afterSequenceNo: 4,
      batchSize: 10
    });

    expect(url).toBe(
      "ws://localhost:62601/ai/agents/runs/7/events/ws?afterSequenceNo=4&batchSize=10&ticket=ws-ticket-1"
    );
  });
});
