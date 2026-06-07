import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiIntegrationsPage from "./page";
import {
  createApiKey,
  createPublicLink,
  listApiKeys,
  listPublicLinks,
  revokeApiKey,
  revokePublicLink
} from "@/api/ai/integration";
import type { ApiKeyResp, PublicLinkResp } from "@/types/ai-integration";

vi.mock("@/api/ai/integration", () => ({
  createApiKey: vi.fn(),
  createPublicLink: vi.fn(),
  listApiKeys: vi.fn(),
  listPublicLinks: vi.fn(),
  revokeApiKey: vi.fn(),
  revokePublicLink: vi.fn()
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

const createApiKeyMock = vi.mocked(createApiKey);
const createPublicLinkMock = vi.mocked(createPublicLink);
const listApiKeysMock = vi.mocked(listApiKeys);
const listPublicLinksMock = vi.mocked(listPublicLinks);
const revokeApiKeyMock = vi.mocked(revokeApiKey);
const revokePublicLinkMock = vi.mocked(revokePublicLink);

function apiKey(overrides: Partial<ApiKeyResp> = {}): ApiKeyResp {
  return {
    id: 123,
    appId: "training_app",
    name: "Training API",
    keyPrefix: "nxk_live",
    maskedKey: "nxk_live_****abcd",
    permissionScope: ["app:training:ask"],
    qpsLimit: 5,
    quotaLimit: 1000,
    expiresAt: "2026-12-31 00:00:00",
    lastUsedAt: null,
    usageSummary: {
      qpsUsed: 1,
      qpsLimit: 5,
      quotaUsed: 37,
      quotaLimit: 1000,
      qpsWindowStart: "2026-06-06 10:00:00",
      quotaWindowStart: "2026-06-01 00:00:00"
    },
    status: 1,
    createTime: "2026-06-06 10:00:00",
    updateTime: null,
    plainKey: null,
    ...overrides
  };
}

function publicLink(overrides: Partial<PublicLinkResp> = {}): PublicLinkResp {
  return {
    id: 456,
    appId: "training_app",
    name: "Training Preview",
    path: "/ask",
    publicUrl: "https://training.example.com/share/abcd",
    maskedToken: "nxl_****abcd",
    permissionScope: ["app:training:ask"],
    qpsLimit: 2,
    quotaLimit: 200,
    expiresAt: "2026-12-31 00:00:00",
    lastUsedAt: null,
    usageSummary: {
      qpsUsed: 1,
      qpsLimit: 2,
      quotaUsed: 12,
      quotaLimit: 200,
      qpsWindowStart: "2026-06-06 10:00:00",
      quotaWindowStart: "2026-06-01 00:00:00"
    },
    status: 1,
    createTime: "2026-06-06 10:00:00",
    updateTime: null,
    ...overrides
  };
}

describe("AiIntegrationsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listApiKeysMock.mockResolvedValue({ list: [apiKey()], total: 1 });
    listPublicLinksMock.mockResolvedValue({ list: [publicLink()], total: 1 });
    createApiKeyMock.mockResolvedValue(apiKey({ id: 124, plainKey: "nxk_live_plain_secret" }));
    createPublicLinkMock.mockResolvedValue(
      publicLink({
        id: 457,
        name: "Training Preview Published",
        publicUrl: "https://training.example.com/share/new-token"
      })
    );
    revokeApiKeyMock.mockResolvedValue(true);
    revokePublicLinkMock.mockResolvedValue(true);
  });

  it("loads, creates, and revokes delivery integration entries", async () => {
    render(<AiIntegrationsPage />);

    expect(await screen.findByText("Training API")).toBeTruthy();
    expect(await screen.findByText("Training Preview")).toBeTruthy();
    expect(await screen.findByText("37 / 1000 quota")).toBeTruthy();
    expect(await screen.findByText("12 / 200 quota")).toBeTruthy();
    await waitFor(() => expect(listApiKeysMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    await waitFor(() => expect(listPublicLinksMock).toHaveBeenCalledWith({ page: 1, size: 20 }));

    fireEvent.click(screen.getByRole("button", { name: "创建 API Key" }));
    await waitFor(() =>
      expect(createApiKeyMock).toHaveBeenCalledWith({
        appId: "training_app",
        name: "Training API",
        permissionScope: ["app:training:ask"],
        qpsLimit: 5,
        quotaLimit: 1000,
        expiresAt: "2026-12-31T00:00:00Z"
      })
    );
    expect(await screen.findByText("nxk_live_plain_secret")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "创建 Public Link" }));
    await waitFor(() =>
      expect(createPublicLinkMock).toHaveBeenCalledWith({
        appId: "training_app",
        name: "Training Preview",
        path: "/ask",
        permissionScope: ["app:training:ask"],
        qpsLimit: 2,
        quotaLimit: 200,
        expiresAt: "2026-12-31T00:00:00Z"
      })
    );
    expect(await screen.findByText("Generated Public URL")).toBeTruthy();
    expect(await screen.findByText("https://training.example.com/share/new-token")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "撤销 API Key" }));
    await waitFor(() => expect(revokeApiKeyMock).toHaveBeenCalledWith(123));
    fireEvent.click(screen.getByRole("button", { name: "撤销 Public Link" }));
    await waitFor(() => expect(revokePublicLinkMock).toHaveBeenCalledWith(456));
  });
});
