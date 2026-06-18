import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Page from "./page";

describe("Codex app POC page", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it("renders the desktop workbench home state", () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        json: async () => ({ code: "200", data: { list: [], total: 0 } })
      }))
    );
    render(<Page />);

    expect(screen.getByRole("heading", { name: "我们应该在当前项目中做些什么？" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "新对话" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "搜索" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "插件" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "自动化" })).toBeTruthy();
    expect(screen.getByText("完全访问")).toBeTruthy();
    expect(screen.getByText("5.5")).toBeTruthy();
    expect(screen.getByText("超高")).toBeTruthy();
  });

  it("opens and closes the slash command menu from the composer", () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        json: async () => ({ code: "200", data: { list: [], total: 0 } })
      }))
    );
    render(<Page />);

    const input = screen.getByLabelText("任务输入");
    fireEvent.change(input, { target: { value: "/" } });

    expect(screen.getByRole("listbox", { name: "命令菜单" })).toBeTruthy();
    expect(screen.getByRole("option", { name: /MCP/ })).toBeTruthy();
    expect(screen.getByRole("option", { name: /个性/ })).toBeTruthy();
    expect(screen.getByRole("option", { name: /推理模式/ })).toBeTruthy();
    expect(screen.getByRole("option", { name: /模型/ })).toBeTruthy();
    expect(screen.getByRole("option", { name: /状态/ })).toBeTruthy();
    expect(screen.getByRole("option", { name: /记忆/ })).toBeTruthy();

    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getByRole("option", { name: /个性/ }).getAttribute("aria-selected")).toBe("true");

    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("listbox", { name: "命令菜单" })).toBeNull();
  });

  it("submits composer input as model loop agent run", async () => {
    const fetchMock = vi.fn(async (url: string) => {
      const href = String(url);
      if (href.includes("/ai/capabilities")) {
        return {
          ok: true,
          json: async () => ({ code: "200", data: { list: [], total: 0 } })
        };
      }
      if (href.includes("/ai/agents/runs") && !href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              runId: 42,
              traceId: "agent-42",
              status: "succeeded",
              finalOutput: "Done"
            }
          })
        };
      }
      if (href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              list: [
                {
                  id: 201,
                  runId: 42,
                  stepId: null,
                  eventType: "thought",
                  sequenceNo: 6,
                  status: "running",
                  payload: {
                    item: {
                      type: "model_delta",
                      routeId: "runtime.llm.code_agent",
                      provider: "openai-compatible",
                      model: "gpt-compatible",
                      deltaIndex: 1,
                      content: " world"
                    }
                  },
                  createTime: "2026-06-17 12:00:01"
                },
                {
                  id: 202,
                  runId: 42,
                  stepId: null,
                  eventType: "thought",
                  sequenceNo: 5,
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
                  createTime: "2026-06-17 12:00:00"
                }
              ],
              total: 2
            }
          })
        };
      }
      return {
        ok: true,
        json: async () => ({ code: "200", data: {} })
      };
    });
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", "runtime.llm.code_agent");

    render(<Page />);
    fireEvent.change(screen.getByLabelText("任务输入"), {
      target: { value: "search policy" }
    });
    fireEvent.click(screen.getByLabelText("发送"));

    expect(fetchMock).toHaveBeenCalled();
    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
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
    expect(await screen.findByText("Done")).toBeTruthy();
    expect(await screen.findByText("Live model output")).toBeTruthy();
    expect(await screen.findByText("Hello world")).toBeTruthy();
  });

  it("shows workbench context controls", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        json: async () => ({ code: "200", data: { list: [], total: 0 } })
      }))
    );

    render(<Page />);

    expect(await screen.findByText("Context")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Files/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Skills/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /MCP/i })).toBeTruthy();
    expect(screen.getByRole("switch", { name: /Web search/i })).toBeTruthy();
  });

  it("submits selected workbench context with a direct question", async () => {
    const fetchMock = vi.fn(async (url: string) => {
      const href = String(url);
      if (href.includes("/ai/capabilities/skills")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              list: [
                {
                  id: 1,
                  code: "support.refund",
                  name: "Refund support",
                  description: "",
                  kind: "skill",
                  status: 1,
                  metadata: {},
                  createTime: ""
                }
              ],
              total: 1
            }
          })
        };
      }
      if (href.includes("/ai/capabilities/mcp/servers")) {
        return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
      }
      if (href.includes("/ai/agents/runs") && !href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: { runId: 7, status: "succeeded", traceId: "agent-7" }
          })
        };
      }
      if (href.includes("/events")) {
        return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
      }
      return { ok: true, json: async () => ({ code: "200", data: {} }) };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.click(await screen.findByRole("button", { name: /Refund support/i }));
    fireEvent.click(screen.getByRole("switch", { name: /Web search/i }));
    fireEvent.change(screen.getByLabelText("任务输入"), {
      target: { value: "Explain the refund policy" }
    });
    fireEvent.click(screen.getByLabelText("发送"));

    const runCall = fetchMock.mock.calls.find(([url]) =>
      String(url).includes("/ai/agents/runs")
    ) as unknown as [string, RequestInit];
    expect(String(runCall[1].body)).toContain('"workbenchContext"');
    expect(String(runCall[1].body)).toContain('"skillCodes":["support.refund"]');
    expect(String(runCall[1].body)).toContain('"webSearchEnabled":true');
  });

  it("uploads a file and includes dataset context in the next run", async () => {
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
      const href = String(url);
      if (href.includes("/ai/knowledge/datasets?name=Codex+Workbench+Inbox")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: { list: [{ id: 7, name: "Codex Workbench Inbox", status: 1 }], total: 1 }
          })
        };
      }
      if (href.includes("/documents/files")) {
        expect(init?.body).toBeInstanceOf(FormData);
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              file: { id: 19, originalName: "handbook.md" },
              parseJob: { id: 29, documentId: 11, status: 2 }
            }
          })
        };
      }
      if (href.includes("/ai/capabilities")) {
        return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
      }
      if (href.includes("/ai/agents/runs") && !href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: { runId: 7, status: "succeeded", traceId: "agent-7" }
          })
        };
      }
      if (href.includes("/events")) {
        return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
      }
      return { ok: true, json: async () => ({ code: "200", data: {} }) };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    const input = await screen.findByLabelText("Upload files");
    fireEvent.change(input, {
      target: { files: [new File(["hello"], "handbook.md", { type: "text/markdown" })] }
    });
    expect(await screen.findByText("handbook.md")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("任务输入"), {
      target: { value: "Summarize the file" }
    });
    fireEvent.click(screen.getByLabelText("发送"));

    const runCall = fetchMock.mock.calls.find(([url]) =>
      String(url).includes("/ai/agents/runs")
    ) as unknown as [string, RequestInit];
    expect(String(runCall[1].body)).toContain('"datasetId":7');
    expect(String(runCall[1].body)).toContain('"documentIds":[11]');
    expect(String(runCall[1].body)).toContain('"fileIds":[19]');
  });

  it("renders readable run evidence from raw events", async () => {
    const fetchMock = vi.fn(async (url: string) => {
      const href = String(url);
      if (href.includes("/ai/agents/runs") && !href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: { runId: 7, status: "succeeded", traceId: "agent-7" }
          })
        };
      }
      if (href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              list: [
                {
                  id: 1,
                  runId: 7,
                  eventType: "thought",
                  sequenceNo: 1,
                  status: "running",
                  payload: { item: { type: "model_delta", content: "Hello" } },
                  createTime: ""
                },
                {
                  id: 2,
                  runId: 7,
                  eventType: "thought",
                  sequenceNo: 2,
                  status: "succeeded",
                  payload: {
                    item: {
                      type: "tool_observation",
                      toolCode: "web.search",
                      output: { dryRun: true, status: "dry_run" }
                    }
                  },
                  createTime: ""
                }
              ],
              total: 2
            }
          })
        };
      }
      if (href.includes("/ai/capabilities")) {
        return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
      }
      return { ok: true, json: async () => ({ code: "200", data: {} }) };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.change(screen.getByLabelText("任务输入"), { target: { value: "Hello" } });
    fireEvent.click(screen.getByLabelText("发送"));

    expect(await screen.findByText("Assistant")).toBeTruthy();
    expect(await screen.findByText("Web search")).toBeTruthy();
    expect(await screen.findByText(/dry-run/i)).toBeTruthy();
  });
});
