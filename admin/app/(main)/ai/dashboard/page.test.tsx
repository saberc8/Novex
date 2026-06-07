import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiDashboardPage from "./page";
import { getCapabilitySummary } from "@/api/ai/capability";
import { getFoundationSummary } from "@/api/ai/foundation";

vi.mock("@/api/ai/capability", () => ({
  getCapabilitySummary: vi.fn()
}));

vi.mock("@/api/ai/foundation", () => ({
  getFoundationSummary: vi.fn()
}));

vi.mock("@/components/permission/permission-gate", () => ({
  PermissionGate: ({ children }: { children: ReactNode }) => <>{children}</>
}));

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn()
  }
}));

const getCapabilitySummaryMock = vi.mocked(getCapabilitySummary);
const getFoundationSummaryMock = vi.mocked(getFoundationSummary);

describe("AiDashboardPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getFoundationSummaryMock.mockResolvedValue({
      status: "skeleton",
      totalModules: 11,
      modules: [
        {
          id: "novex-model",
          name: "Model Runtime",
          layer: "ai-foundation",
          status: "skeleton",
          description: "Model routing and adapters"
        },
        {
          id: "novex-rag",
          name: "RAG",
          layer: "ai-foundation",
          status: "skeleton",
          description: "Chunking and retrieval"
        }
      ],
      milestoneCoverage: [
        {
          id: "M1",
          name: "Knowledge Base MVP",
          status: "poc_limited",
          summary: "RAG query path, citations, trace, and training/chat-web pages are present.",
          evidence: ["Admin knowledge control plane"],
          limitations: ["Milvus is wired through metadata but live POC still supports local fallback."]
        }
      ]
    });
    getCapabilitySummaryMock.mockResolvedValue({
      skillCount: 2,
      toolCount: 3,
      connectorCount: 2,
      pluginCount: 2,
      triggerCount: 1,
      mcpServerCount: 1
    });
  });

  it("loads foundation modules and capability governance counts", async () => {
    render(<AiDashboardPage />);

    await waitFor(() => expect(getFoundationSummaryMock).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(getCapabilitySummaryMock).toHaveBeenCalledTimes(1));

    expect(await screen.findByRole("heading", { name: "AI 基座总览", level: 1 })).toBeTruthy();
    expect(await screen.findByText("novex-model")).toBeTruthy();
    expect(await screen.findByText("novex-rag")).toBeTruthy();
    expect((await screen.findAllByText("11")).length).toBeGreaterThan(0);
    expect(await screen.findByText("Tools")).toBeTruthy();
    expect((await screen.findAllByText("3")).length).toBeGreaterThan(0);
    expect(await screen.findByText("MCP Servers")).toBeTruthy();
    expect((await screen.findAllByText("1")).length).toBeGreaterThan(0);
    expect(await screen.findByText("M0-M5 Coverage")).toBeTruthy();
    expect(await screen.findByText("Knowledge Base MVP")).toBeTruthy();
    expect(await screen.findByText("poc_limited")).toBeTruthy();
    expect(await screen.findByText(/Milvus is wired/)).toBeTruthy();
    expect(screen.queryByText("Next Milestone")).toBeNull();
  });
});
