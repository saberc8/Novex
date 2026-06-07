import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import IdentityProvidersPage from "./page";
import { listIdentityProviders } from "@/api/system/identity";
import type { IdentityProviderResp } from "@/types/system-identity";

vi.mock("@/api/system/identity", () => ({
  listIdentityProviders: vi.fn()
}));

vi.mock("@/components/permission/permission-gate", () => ({
  PermissionGate: ({ children }: { children: ReactNode }) => <>{children}</>
}));

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn()
  }
}));

const listIdentityProvidersMock = vi.mocked(listIdentityProviders);

function provider(overrides: Partial<IdentityProviderResp> = {}): IdentityProviderResp {
  return {
    id: 1097001,
    tenantId: 1,
    providerType: "github",
    code: "github.login",
    name: "GitHub Login",
    clientId: "github-client",
    secretRef: "env:GITHUB_OAUTH_CLIENT_SECRET",
    maskedSecretRef: "env:GITH****",
    allowedDomains: [],
    tenantPolicy: {
      defaultScopes: ["read:user", "user:email"],
      credentialBoundary: "login_identity_only_not_repo_connector"
    },
    status: 1,
    createTime: "2026-06-06 10:00:00",
    ...overrides
  };
}

describe("IdentityProvidersPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listIdentityProvidersMock.mockResolvedValue({ list: [provider()], total: 1 });
  });

  it("renders GitHub identity provider without treating it as a connector credential", async () => {
    render(<IdentityProvidersPage />);

    expect(await screen.findByRole("heading", { name: "身份源" })).toBeTruthy();
    expect(await screen.findByText("GitHub Login")).toBeTruthy();
    expect(await screen.findByText("github.login")).toBeTruthy();
    expect(await screen.findByText("env:GITH****")).toBeTruthy();
    expect(await screen.findByText("login_identity_only_not_repo_connector")).toBeTruthy();
    await waitFor(() => expect(listIdentityProvidersMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
  });
});
