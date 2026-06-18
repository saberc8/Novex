import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Page from "./page";

describe("Codex app POC page", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders the desktop workbench home state", () => {
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
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({
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
      })
      .mockResolvedValueOnce({
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
});
