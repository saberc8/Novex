import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Page from "./page";
import {
  cancelAgentRun,
  createAgentRun,
  listAgentRunEvents,
  listAgentRuns,
  resumeAgentRun
} from "@/api/agent";
import type { AgentRunEventResp, AgentRunResp } from "@/types/agent";

vi.mock("@/api/agent", () => ({
  cancelAgentRun: vi.fn(),
  createAgentRun: vi.fn(),
  listAgentRunEvents: vi.fn(),
  listAgentRuns: vi.fn(),
  resumeAgentRun: vi.fn()
}));

const cancelAgentRunMock = vi.mocked(cancelAgentRun);
const createAgentRunMock = vi.mocked(createAgentRun);
const listAgentRunEventsMock = vi.mocked(listAgentRunEvents);
const listAgentRunsMock = vi.mocked(listAgentRuns);
const resumeAgentRunMock = vi.mocked(resumeAgentRun);

function run(overrides: Partial<AgentRunResp> = {}): AgentRunResp {
  return {
    runId: 42,
    traceId: "trace-42",
    status: "waiting_approval",
    intent: "tool_task",
    loopKind: "react",
    selectedToolCode: "feishu.message.send",
    pauseReason: "approval",
    finalOutput: null,
    taskBudget: {
      maxSteps: 6,
      maxToolCalls: 1,
      maxSeconds: 30,
      maxCostCents: 0
    },
    createTime: "2026-06-05 12:00:00",
    updateTime: null,
    ...overrides
  };
}

function event(overrides: Partial<AgentRunEventResp> = {}): AgentRunEventResp {
  return {
    id: 100,
    runId: 42,
    stepId: 90,
    eventType: "approval_requested",
    sequenceNo: 3,
    status: "waiting_approval",
    payload: {
      toolCode: "feishu.message.send",
      riskLevel: 2
    },
    createTime: "2026-06-05 12:00:01",
    ...overrides
  };
}

describe("Agent workspace page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listAgentRunsMock.mockResolvedValue({
      list: [run()],
      total: 1
    });
    listAgentRunEventsMock.mockResolvedValue({
      list: [event()],
      total: 1
    });
    createAgentRunMock.mockResolvedValue(run());
    resumeAgentRunMock.mockResolvedValue(
      run({
        status: "succeeded",
        pauseReason: null,
        finalOutput: "Agent dry-run executed feishu.message.send."
      })
    );
    cancelAgentRunMock.mockResolvedValue(
      run({
        status: "cancelled",
        pauseReason: null,
        finalOutput: "Agent run cancelled."
      })
    );
  });

  it("renders a customer-facing agent workspace surface", async () => {
    render(<Page />);

    expect(screen.getByRole("heading", { name: "Novex Agent", level: 1 })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Runs" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Workflow" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Tools" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Memory" })).toBeTruthy();
    await waitFor(() => expect(listAgentRunsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    expect(await screen.findByText("feishu.message.send")).toBeTruthy();
    expect(await screen.findByText("approval_requested")).toBeTruthy();
  });

  it("creates a run and refreshes the event snapshot", async () => {
    render(<Page />);

    await waitFor(() => expect(listAgentRunsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    fireEvent.change(screen.getByLabelText("Describe the task"), {
      target: { value: "send Feishu training reminder" }
    });
    fireEvent.click(screen.getByRole("button", { name: "Start run" }));

    await waitFor(() =>
      expect(createAgentRunMock).toHaveBeenCalledWith({
        input: "send Feishu training reminder",
        autoApprove: false,
        budget: {
          maxSteps: 6,
          maxToolCalls: 1,
          maxSeconds: 30,
          maxCostCents: 0
        }
      })
    );
    expect(listAgentRunEventsMock).toHaveBeenCalledWith(42, { page: 1, size: 100 });
  });

  it("approves a paused run from the workspace", async () => {
    render(<Page />);

    await screen.findByText("approval_requested");
    fireEvent.click(screen.getByRole("button", { name: "Approve run" }));

    await waitFor(() =>
      expect(resumeAgentRunMock).toHaveBeenCalledWith(42, {
        approved: true,
        input: { source: "agent-workspace" }
      })
    );
    expect(await screen.findByText("Agent dry-run executed feishu.message.send.")).toBeTruthy();
  });

  it("cancels an active run from the workspace", async () => {
    render(<Page />);

    await screen.findByText("approval_requested");
    fireEvent.click(screen.getByRole("button", { name: "Cancel run" }));

    await waitFor(() => expect(cancelAgentRunMock).toHaveBeenCalledWith(42));
    expect(await screen.findByText("Agent run cancelled.")).toBeTruthy();
  });
});
