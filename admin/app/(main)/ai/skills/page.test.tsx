import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiSkillsPage from "./page";
import { listSkills } from "@/api/ai/capability";
import type { CapabilityItemResp } from "@/types/ai-capability";

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

vi.mock("@/components/permission/permission-gate", () => ({
  PermissionGate: ({ children }: { children: ReactNode }) => <>{children}</>
}));

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn()
  }
}));

const listSkillsMock = vi.mocked(listSkills);

function skill(overrides: Partial<CapabilityItemResp> = {}): CapabilityItemResp {
  return {
    id: 3200104,
    code: "training_quiz",
    name: "Training Quiz",
    description: "Builds quizzes from cited training content.",
    kind: "skill",
    status: 1,
    riskLevel: null,
    metadata: {
      modelRoutePolicy: {
        answerModel: "runtime.llm.rag_answer",
        embeddingModel: "runtime.embedding.default",
        rerankModel: "runtime.rerank.default"
      },
      capabilityRefs: [
        { kind: "tool", code: "rag.search" }
      ],
      template: "training_app",
      evalCases: ["training_regression"]
    },
    createTime: "2026-06-06 10:00:00",
    ...overrides
  };
}

describe("AiSkillsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listSkillsMock.mockResolvedValue({ list: [skill()], total: 1 });
  });

  it("loads skill manifests through the capability registry", async () => {
    render(<AiSkillsPage />);

    expect(await screen.findByRole("heading", { name: "Skills" })).toBeTruthy();
    expect(await screen.findByText("Training Quiz")).toBeTruthy();
    expect(await screen.findByText("training_quiz")).toBeTruthy();
    expect(await screen.findByText("runtime.llm.rag_answer")).toBeTruthy();
    expect(await screen.findByText("rag.search")).toBeTruthy();
    expect(await screen.findByText("ai:skill:list")).toBeTruthy();
    await waitFor(() => expect(listSkillsMock).toHaveBeenCalledWith({ page: 1, size: 50 }));
  });
});
