import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiMemoryPage from "./page";
import { deleteMemory, listMemories, upsertMemory } from "@/api/ai/memory";
import type { MemoryResp } from "@/types/ai-memory";

vi.mock("@/api/ai/memory", () => ({
  deleteMemory: vi.fn(),
  listMemories: vi.fn(),
  upsertMemory: vi.fn()
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

const deleteMemoryMock = vi.mocked(deleteMemory);
const listMemoriesMock = vi.mocked(listMemories);
const upsertMemoryMock = vi.mocked(upsertMemory);

function memory(overrides: Partial<MemoryResp> = {}): MemoryResp {
  return {
    id: 42,
    scopeType: "user",
    scopeId: "1",
    sourceKind: "manual",
    sourceId: "note-7",
    content: "prefers concise updates",
    summary: "concise updates",
    sensitivity: "preference",
    writePolicy: "user_approved",
    ttlDays: 90,
    expiresAt: "2026-09-04 10:00:00",
    metadata: { confirmedByUser: true },
    status: 1,
    createTime: "2026-06-06 10:00:00",
    updateTime: null,
    ...overrides
  };
}

describe("AiMemoryPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMemoriesMock.mockResolvedValue({ list: [memory()], total: 1 });
    upsertMemoryMock.mockResolvedValue(memory({ id: 43 }));
    deleteMemoryMock.mockResolvedValue(true);
  });

  it("loads memories, saves a user-approved memory, and deletes an entry", async () => {
    render(<AiMemoryPage />);

    expect(await screen.findByText("concise updates")).toBeTruthy();
    await waitFor(() => expect(listMemoriesMock).toHaveBeenCalledWith({ page: 1, size: 20 }));

    fireEvent.change(screen.getByPlaceholderText("prefers concise updates"), {
      target: { value: "prefers concise updates" }
    });
    fireEvent.click(screen.getByRole("button", { name: "保存记忆" }));

    await waitFor(() =>
      expect(upsertMemoryMock).toHaveBeenCalledWith({
        scopeType: "user",
        scopeId: "1",
        sourceKind: "manual",
        sourceId: "admin-note",
        content: "prefers concise updates",
        summary: "prefers concise updates",
        sensitivity: "preference",
        writePolicy: "user_approved",
        ttlDays: 90,
        metadata: { confirmedByUser: true },
        status: 1
      })
    );

    fireEvent.click(screen.getByRole("button", { name: "删除" }));

    await waitFor(() => expect(deleteMemoryMock).toHaveBeenCalledWith(42));
  });
});
