import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiTracesPage from "./page";
import { listAgentRunEvents, listAgentRuns } from "@/api/ai/agent";
import type { AgentRunEventResp, AgentRunResp } from "@/types/ai-agent";

vi.mock("@/api/ai/agent", () => ({
  listAgentRunEvents: vi.fn(),
  listAgentRuns: vi.fn()
}));

vi.mock("@/components/permission/permission-gate", () => ({
  PermissionGate: ({ children }: { children: ReactNode }) => <>{children}</>
}));

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn()
  }
}));

const listAgentRunEventsMock = vi.mocked(listAgentRunEvents);
const listAgentRunsMock = vi.mocked(listAgentRuns);

function run(overrides: Partial<AgentRunResp> = {}): AgentRunResp {
  return {
    runId: 42,
    traceId: "agent-42",
    status: "waiting_approval",
    intent: "training_reminder",
    loopKind: "react",
    selectedToolCode: "feishu.message.send",
    pauseReason: "tool_approval",
    finalOutput: null,
    taskBudget: {
      maxSteps: 6,
      maxToolCalls: 2,
      maxSeconds: 30,
      maxCostCents: 0
    },
    createTime: "2026-06-05 12:00:00",
    updateTime: "2026-06-05 12:00:01",
    ...overrides
  };
}

function event(overrides: Partial<AgentRunEventResp> = {}): AgentRunEventResp {
  return {
    id: 100,
    runId: 42,
    stepId: 80,
    eventType: "approval_requested",
    sequenceNo: 3,
    status: "pending",
    payload: {
      toolCode: "feishu.message.send",
      recipient: "training-team"
    },
    createTime: "2026-06-05 12:00:01",
    ...overrides
  };
}

describe("AiTracesPage", () => {
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
  });

  it("shows agent run trace metadata and replay event snapshot", async () => {
    render(<AiTracesPage />);

    await waitFor(() =>
      expect(listAgentRunsMock).toHaveBeenCalledWith({
        page: 1,
        size: 20
      })
    );
    await waitFor(() => expect(listAgentRunEventsMock).toHaveBeenCalledWith(42, { page: 1, size: 100 }));

    expect(await screen.findByText("Run Trace")).toBeTruthy();
    expect(screen.getAllByText("agent-42").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("tool_approval")).toBeTruthy();
    expect(screen.getAllByText("feishu.message.send").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Event Replay Snapshot")).toBeTruthy();
    expect(screen.getByText("approval_requested")).toBeTruthy();
    expect(screen.getByText(/training-team/)).toBeTruthy();
  });
});
