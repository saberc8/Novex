import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiConnectorsPage from "./page";
import {
  listConnectorCredentials,
  listConnectors,
  upsertConnectorCredential
} from "@/api/ai/capability";
import type { CapabilityItemResp, ConnectorCredentialResp } from "@/types/ai-capability";

vi.mock("@/api/ai/capability", () => ({
  dryRunTool: vi.fn(),
  listConnectorCredentials: vi.fn(),
  listConnectors: vi.fn(),
  listMcpServers: vi.fn(),
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

const listConnectorCredentialsMock = vi.mocked(listConnectorCredentials);
const listConnectorsMock = vi.mocked(listConnectors);
const upsertConnectorCredentialMock = vi.mocked(upsertConnectorCredential);

function connector(overrides: Partial<CapabilityItemResp> = {}): CapabilityItemResp {
  return {
    id: 1,
    code: "github.default",
    name: "GitHub",
    description: "Repository search and file read connector",
    kind: "source_control",
    status: 1,
    riskLevel: null,
    metadata: {},
    createTime: "2026-06-06 10:00:00",
    ...overrides
  };
}

function credential(overrides: Partial<ConnectorCredentialResp> = {}): ConnectorCredentialResp {
  return {
    id: 10,
    connectorId: 1,
    connectorCode: "github.default",
    scopeType: "tenant",
    scopeId: "1",
    authType: "oauth_app",
    secretRef: "env:GITHUB_CONNECTOR_TOKEN",
    maskedValue: "env:GITH****",
    scopes: ["repo"],
    status: 1,
    createTime: "2026-06-06 10:05:00",
    updateTime: null,
    ...overrides
  };
}

describe("AiConnectorsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listConnectorsMock.mockResolvedValue({ list: [connector()], total: 1 });
    listConnectorCredentialsMock.mockResolvedValue({ list: [credential()], total: 1 });
    upsertConnectorCredentialMock.mockResolvedValue(credential({ id: 11, scopes: ["repo", "read:org"] }));
  });

  it("loads connector credentials and submits an env-backed credential binding", async () => {
    render(<AiConnectorsPage />);

    expect(await screen.findByText("GitHub")).toBeTruthy();
    expect(await screen.findByText("env:GITH****")).toBeTruthy();
    await waitFor(() => expect(listConnectorsMock).toHaveBeenCalledWith({ page: 1, size: 50 }));
    await waitFor(() =>
      expect(listConnectorCredentialsMock).toHaveBeenCalledWith({ page: 1, size: 20 })
    );

    fireEvent.change(screen.getByPlaceholderText("github.default"), {
      target: { value: "github.default" }
    });
    fireEvent.change(screen.getByPlaceholderText("1"), {
      target: { value: "1" }
    });
    fireEvent.change(screen.getByPlaceholderText("oauth_app"), {
      target: { value: "oauth_app" }
    });
    fireEvent.change(screen.getByPlaceholderText("env:GITHUB_CONNECTOR_TOKEN"), {
      target: { value: "env:GITHUB_CONNECTOR_TOKEN" }
    });
    fireEvent.change(screen.getByPlaceholderText("repo, read:org"), {
      target: { value: "repo, read:org" }
    });
    fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));

    await waitFor(() =>
      expect(upsertConnectorCredentialMock).toHaveBeenCalledWith({
        connectorCode: "github.default",
        scopeType: "tenant",
        scopeId: "1",
        authType: "oauth_app",
        secretRef: "env:GITHUB_CONNECTOR_TOKEN",
        scopes: ["repo", "read:org"],
        status: 1
      })
    );
  });
});
