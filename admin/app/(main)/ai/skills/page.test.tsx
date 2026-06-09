import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiSkillsPage from "./page";
import {
  importSkill,
  importSkillFromSource,
  listSkills,
  previewSkillImport
} from "@/api/ai/capability";
import type { CapabilityItemResp } from "@/types/ai-capability";

vi.mock("@/api/ai/capability", () => ({
  dryRunTool: vi.fn(),
  importSkill: vi.fn(),
  importSkillFromSource: vi.fn(),
  importSkillPackage: vi.fn(),
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
  previewSkillImport: vi.fn(),
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
const importSkillMock = vi.mocked(importSkill);
const previewSkillImportMock = vi.mocked(previewSkillImport);
const importSkillFromSourceMock = vi.mocked(importSkillFromSource);

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
    importSkillMock.mockResolvedValue(skill({ code: "codex_skill", name: "Codex Skill" }));
    previewSkillImportMock.mockResolvedValue({
      sourceUrl: "https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer",
      skills: [
        {
          code: "khazix-writer",
          name: "khazix-writer",
          description: "Generate cited long-form content.",
          path: "khazix-writer",
          referenceCount: 2,
          scriptCount: 0,
          assetCount: 0
        }
      ],
      warnings: []
    });
    importSkillFromSourceMock.mockResolvedValue({
      skill: skill({ code: "khazix-writer", name: "khazix-writer" }),
      resourceCount: 3,
      referenceCount: 2,
      scriptCount: 0,
      assetCount: 0,
      warnings: []
    });
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

  it("imports a Codex SKILL.md file and refreshes the skill registry", async () => {
    render(<AiSkillsPage />);

    expect(await screen.findByText("Training Quiz")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "导入 Skill" }));
    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File(
      [
        "---\nname: codex_skill\ndescription: Imported skill.\n---\n# Codex Skill\nUse cited context."
      ],
      "SKILL.md",
      { type: "text/markdown" }
    );

    fireEvent.change(input, { target: { files: [file] } });

    await waitFor(() => expect(importSkillMock).toHaveBeenCalledTimes(1));
    const formData = importSkillMock.mock.calls[0]?.[0] as FormData;
    expect(formData.get("file")).toBe(file);
    await waitFor(() => expect(listSkillsMock).toHaveBeenCalledTimes(2));
  });

  it("previews and installs skills from the AI import drawer", async () => {
    render(<AiSkillsPage />);

    expect(await screen.findByText("Training Quiz")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /AI 导入 Skills/ }));
    fireEvent.change(screen.getByLabelText("导入需求或 GitHub 地址"), {
      target: {
        value: "https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer"
      }
    });
    fireEvent.click(screen.getByRole("button", { name: "分析" }));

    await waitFor(() => expect(previewSkillImportMock).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getAllByText("khazix-writer").length).toBeGreaterThan(0));
    expect(screen.getByText("references 2")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "安装" }));

    await waitFor(() =>
      expect(importSkillFromSourceMock).toHaveBeenCalledWith({
        source: "https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer",
        skillPath: "khazix-writer"
      })
    );
    await waitFor(() => expect(listSkillsMock).toHaveBeenCalledTimes(2));
  });
});
