import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiPluginsPage from "./page";
import { installPlugin, listPluginInstallations, listPlugins } from "@/api/ai/capability";
import type { CapabilityItemResp, PluginInstallationResp } from "@/types/ai-capability";

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
  upsertConnectorCredential: vi.fn()
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

const installPluginMock = vi.mocked(installPlugin);
const listPluginInstallationsMock = vi.mocked(listPluginInstallations);
const listPluginsMock = vi.mocked(listPlugins);

function plugin(overrides: Partial<CapabilityItemResp> = {}): CapabilityItemResp {
  return {
    id: 3230001,
    code: "builtin.github-basic",
    name: "GitHub Basic",
    description: "GitHub repo search/read plugin",
    kind: "builtin_adapter",
    status: 1,
    riskLevel: null,
    metadata: {
      version: "0.1.0",
      manifest: {
        permissions: ["ai:connector:list", "ai:tool:dryRun"]
      }
    },
    createTime: "2026-06-06 10:00:00",
    ...overrides
  };
}

function installation(overrides: Partial<PluginInstallationResp> = {}): PluginInstallationResp {
  return {
    id: 3233001,
    pluginId: 3230001,
    pluginCode: "builtin.github-basic",
    pluginName: "GitHub Basic",
    version: "0.1.0",
    enabled: true,
    permissionGrants: ["ai:connector:list", "ai:tool:dryRun"],
    capabilities: [{ kind: "tool", code: "github.repo.search", permissionCode: "ai:tool:dryRun" }],
    config: {},
    installSource: "builtin",
    createTime: "2026-06-06 10:05:00",
    updateTime: null,
    ...overrides
  };
}

describe("AiPluginsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listPluginsMock.mockResolvedValue({ list: [plugin()], total: 1 });
    listPluginInstallationsMock.mockResolvedValueOnce({ list: [], total: 0 });
    listPluginInstallationsMock.mockResolvedValue({ list: [installation()], total: 1 });
    installPluginMock.mockResolvedValue(installation());
  });

  it("loads plugin installation state and enables a builtin plugin", async () => {
    render(<AiPluginsPage />);

    expect(await screen.findByText("GitHub Basic")).toBeTruthy();
    await waitFor(() => expect(listPluginsMock).toHaveBeenCalledWith({ page: 1, size: 50 }));
    await waitFor(() =>
      expect(listPluginInstallationsMock).toHaveBeenCalledWith({ page: 1, size: 50 })
    );

    fireEvent.click(screen.getByRole("button", { name: "启用插件" }));

    await waitFor(() =>
      expect(installPluginMock).toHaveBeenCalledWith({
        pluginCode: "builtin.github-basic",
        version: "0.1.0",
        enabled: true,
        permissionGrants: ["ai:connector:list", "ai:tool:dryRun"],
        config: {}
      })
    );
  });
});
