import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import KnowledgePage from "./page";
import { createDataset, listDatasets, listDocuments } from "@/api/ai/knowledge";
import type { DatasetResp, DocumentResp } from "@/types/ai";

vi.mock("@/api/ai/knowledge", () => ({
  createDataset: vi.fn(),
  listDatasets: vi.fn(),
  listDocuments: vi.fn()
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

const createDatasetMock = vi.mocked(createDataset);
const listDatasetsMock = vi.mocked(listDatasets);
const listDocumentsMock = vi.mocked(listDocuments);

function dataset(overrides: Partial<DatasetResp>): DatasetResp {
  return {
    id: 10,
    tenantId: 1,
    name: "员工手册",
    description: "制度与培训资料",
    ownerId: 1,
    visibility: 1,
    status: 1,
    retrievalMode: 3,
    documentCount: 2,
    chunkCount: 18,
    createUserString: "admin",
    createTime: "2026-06-05 10:00:00",
    updateUserString: "",
    updateTime: "",
    ...overrides
  };
}

function document(overrides: Partial<DocumentResp>): DocumentResp {
  return {
    id: 20,
    tenantId: 1,
    datasetId: 10,
    name: "入职流程.pdf",
    sourceUri: "",
    fileId: 100,
    contentType: "application/pdf",
    ownerId: 1,
    visibility: 1,
    parseStatus: 1,
    ingestionStatus: 1,
    chunkCount: 0,
    sourceHash: "",
    createUserString: "admin",
    createTime: "2026-06-05 10:10:00",
    updateUserString: "",
    updateTime: "",
    ...overrides
  };
}

describe("KnowledgePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    createDatasetMock.mockResolvedValue(99);
    listDatasetsMock.mockResolvedValue({
      list: [dataset({ id: 10 }), dataset({ id: 11, name: "产品资料", documentCount: 0 })],
      total: 2
    });
    listDocumentsMock.mockResolvedValue({ list: [document({ id: 20 })], total: 1 });
  });

  it("keeps dataset and document panels aligned and loads documents for the selected dataset", async () => {
    render(<KnowledgePage />);

    const layout = await screen.findByTestId("knowledge-layout");
    expect(layout.className).toContain("items-start");
    expect(screen.getByTestId("dataset-list-panel").className).toContain("self-start");
    expect(screen.getByTestId("documents-panel").className).toContain("self-start");
    expect(screen.getByTestId("documents-panel").className).toContain("content-start");
    expect(await screen.findByTestId("dataset-card-10")).toBeTruthy();
    expect(await screen.findByText("入职流程.pdf")).toBeTruthy();
    await waitFor(() =>
      expect(listDocumentsMock).toHaveBeenCalledWith(10, {
        page: 1,
        size: 20
      })
    );
  });

  it("submits a new dataset with default visibility and retrieval mode", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: "新增知识库" }));
    fireEvent.change(screen.getByPlaceholderText("知识库名称"), {
      target: { value: "研发制度" }
    });
    fireEvent.change(screen.getByPlaceholderText("描述这个知识库的内容范围"), {
      target: { value: "研发流程与规范" }
    });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() =>
      expect(createDatasetMock).toHaveBeenCalledWith({
        name: "研发制度",
        description: "研发流程与规范",
        visibility: 1,
        retrievalMode: 3
      })
    );
  });
});
