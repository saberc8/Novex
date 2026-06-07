import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiTriggersPage from "./page";
import { listTriggers } from "@/api/ai/capability";
import { listTriggerEvents } from "@/api/ai/trigger";
import type { CapabilityItemResp } from "@/types/ai-capability";
import type { TriggerEventResp } from "@/types/ai-trigger";

vi.mock("@/api/ai/capability", () => ({
  dryRunTool: vi.fn(),
  installPlugin: vi.fn(),
  listConnectorCredentials: vi.fn(),
  listConnectors: vi.fn(),
  listMcpServers: vi.fn(),
  listPluginInstallations: vi.fn(),
  listPlugins: vi.fn(),
  listSkills: vi.fn(),
  listToolAudits: vi.fn(),
  listTools: vi.fn(),
  listTriggers: vi.fn(),
  upsertConnectorCredential: vi.fn(),
  upsertMcpServer: vi.fn()
}));

vi.mock("@/api/ai/trigger", () => ({
  listTriggerEvents: vi.fn()
}));

vi.mock("@/components/permission/permission-gate", () => ({
  PermissionGate: ({ children }: { children: ReactNode }) => <>{children}</>
}));

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn()
  }
}));

const listTriggersMock = vi.mocked(listTriggers);
const listTriggerEventsMock = vi.mocked(listTriggerEvents);

function trigger(overrides: Partial<CapabilityItemResp> = {}): CapabilityItemResp {
  return {
    id: 3240001,
    code: "webhook.training.event",
    name: "Training Webhook",
    description: "Signed webhook POC for training events.",
    kind: "webhook",
    status: 1,
    riskLevel: null,
    metadata: {
      path: "/ai/triggers/webhook/training"
    },
    createTime: "2026-06-06 10:00:00",
    ...overrides
  };
}

function triggerEvent(overrides: Partial<TriggerEventResp> = {}): TriggerEventResp {
  return {
    id: 9001,
    triggerCode: "webhook.training.event",
    sourceType: "webhook",
    targetKind: "run_graph",
    idempotencyKey: "tenant-1:event-7",
    status: "accepted",
    traceId: 9001,
    errorMessage: null,
    eventPayload: { event: "training.completed", employeeId: 7 },
    routeSnapshot: { targetKind: "run_graph" },
    createTime: "2026-06-06 10:05:00",
    ...overrides
  };
}

describe("AiTriggersPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listTriggersMock.mockResolvedValue({ list: [trigger()], total: 1 });
    listTriggerEventsMock.mockResolvedValue({ list: [triggerEvent()], total: 1 });
  });

  it("loads trigger registry and webhook event audit records", async () => {
    render(<AiTriggersPage />);

    expect(await screen.findByText("Training Webhook")).toBeTruthy();
    expect(await screen.findByText("Webhook Events")).toBeTruthy();
    expect(await screen.findByText("tenant-1:event-7")).toBeTruthy();
    expect(await screen.findByText("training.completed")).toBeTruthy();
    expect(await screen.findByText("Trace #9001")).toBeTruthy();
    await waitFor(() => expect(listTriggersMock).toHaveBeenCalledWith({ page: 1, size: 50 }));
    await waitFor(() => expect(listTriggerEventsMock).toHaveBeenCalledWith({ page: 1, size: 10 }));
  });
});
