import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { resolvePublicShare } from "@/api/integration";
import { PublicShareClient } from "./public-share-client";

vi.mock("@/api/integration", () => ({
  resolvePublicShare: vi.fn()
}));

const resolvePublicShareMock = vi.mocked(resolvePublicShare);

describe("PublicShareClient", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resolvePublicShareMock.mockResolvedValue({
      accepted: true,
      targetPath: "/chat",
      auth: {
        principalType: "publicLink",
        tenantId: 1,
        appId: "llm_chat",
        name: "Published Chat",
        path: "/chat",
        maskedCredential: "nxl_****1234",
        permissionScope: ["app:chat:use", "ai:model:chat"],
        qpsLimit: 2,
        quotaLimit: 200,
        expiresAt: "2026-12-31 00:00:00",
        lastUsedAt: null
      }
    });
  });

  it("renders the public link runtime context for the share token", async () => {
    render(<PublicShareClient token="nxl_public_token_1234" />);

    await waitFor(() => expect(resolvePublicShareMock).toHaveBeenCalledWith("nxl_public_token_1234"));
    expect(await screen.findByRole("heading", { name: "Novex Share", level: 1 })).toBeTruthy();
    expect(await screen.findByText("Published Chat")).toBeTruthy();
    expect(await screen.findByText("llm_chat")).toBeTruthy();
    expect(await screen.findByText("/chat")).toBeTruthy();
    expect(await screen.findByText("2 QPS / 200 quota")).toBeTruthy();
    expect(await screen.findByText("app:chat:use")).toBeTruthy();
    expect(await screen.findByText("nxl_****1234")).toBeTruthy();
    expect(screen.queryByText("nxl_public_token_1234")).toBeNull();
  });

  it("shows an invalid share state without exposing the raw token", async () => {
    resolvePublicShareMock.mockRejectedValueOnce(new Error("bad token"));

    render(<PublicShareClient token="bad-token" />);

    expect(await screen.findByText("Share link unavailable")).toBeTruthy();
    expect(await screen.findByText("Token hidden")).toBeTruthy();
    expect(screen.queryByText("bad-token")).toBeNull();
  });
});
