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
    const fetchMock = vi.fn(async () => ({
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
    }));
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);
    fireEvent.change(screen.getByLabelText("任务输入"), {
      target: { value: "search policy" }
    });
    fireEvent.click(screen.getByLabelText("发送"));

    expect(fetchMock).toHaveBeenCalled();
    expect(await screen.findByText("Done")).toBeTruthy();
  });
});
