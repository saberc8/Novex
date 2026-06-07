import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiMcpPage from "./page";
import { listMcpServers, upsertMcpServer } from "@/api/ai/capability";
import type { CapabilityItemResp, McpServerResp } from "@/types/ai-capability";

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

const listMcpServersMock = vi.mocked(listMcpServers);
const upsertMcpServerMock = vi.mocked(upsertMcpServer);

function mcpServer(overrides: Partial<CapabilityItemResp> = {}): CapabilityItemResp {
  return {
    id: 3250001,
    code: "local.dry-run",
    name: "Local Dry Run MCP",
    description: "",
    kind: "tenant",
    status: 1,
    riskLevel: null,
    metadata: {
      transportKind: "builtin",
      authType: "none",
      discoveredTools: [{ name: "rag.search", permissionCode: "ai:knowledge:ask" }]
    },
    createTime: "2026-06-06 10:00:00",
    ...overrides
  };
}

function mcpResponse(overrides: Partial<McpServerResp> = {}): McpServerResp {
  return {
    id: 3250002,
    code: "docs.search",
    name: "Docs Search",
    endpointUrl: "https://mcp.example.com/sse",
    transportKind: "streamable_http",
    authScope: "tenant",
    authType: "bearer_env",
    secretRef: "env:DOCS_MCP_TOKEN",
    maskedSecretRef: "env:DOCS****",
    networkAllowlist: ["mcp.example.com"],
    toolAllowlist: ["docs.search"],
    discoveredTools: [],
    enabled: true,
    createTime: "2026-06-06 10:05:00",
    updateTime: null,
    ...overrides
  };
}

describe("AiMcpPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMcpServersMock.mockResolvedValue({ list: [mcpServer()], total: 1 });
    upsertMcpServerMock.mockResolvedValue(mcpResponse());
  });

  it("loads MCP servers and registers an allow-listed HTTP server", async () => {
    render(<AiMcpPage />);

    expect(await screen.findByText("Local Dry Run MCP")).toBeTruthy();
    await waitFor(() => expect(listMcpServersMock).toHaveBeenCalledWith({ page: 1, size: 50 }));

    fireEvent.change(screen.getByPlaceholderText("docs.search"), {
      target: { value: "docs.search" }
    });
    fireEvent.change(screen.getByPlaceholderText("Docs Search"), {
      target: { value: "Docs Search" }
    });
    fireEvent.change(screen.getByPlaceholderText("https://mcp.example.com/sse"), {
      target: { value: "https://mcp.example.com/sse" }
    });
    fireEvent.change(screen.getByPlaceholderText("env:DOCS_MCP_TOKEN"), {
      target: { value: "env:DOCS_MCP_TOKEN" }
    });
    fireEvent.change(screen.getByPlaceholderText("mcp.example.com"), {
      target: { value: "mcp.example.com" }
    });
    fireEvent.change(screen.getByPlaceholderText("docs.search, docs.read"), {
      target: { value: "docs.search" }
    });
    fireEvent.click(screen.getByRole("button", { name: "保存 MCP Server" }));

    await waitFor(() =>
      expect(upsertMcpServerMock).toHaveBeenCalledWith({
        code: "docs.search",
        name: "Docs Search",
        endpointUrl: "https://mcp.example.com/sse",
        transportKind: "streamable_http",
        authScope: "tenant",
        authType: "bearer_env",
        secretRef: "env:DOCS_MCP_TOKEN",
        networkAllowlist: ["mcp.example.com"],
        toolAllowlist: ["docs.search"],
        discoveredTools: [],
        enabled: true
      })
    );
  });
});
