import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Page from "./page";
import { askDataset, listDatasets, submitRagFeedback } from "@/api/knowledge";
import type { DatasetResp } from "@/types/knowledge";

vi.mock("@/api/knowledge", () => ({
  askDataset: vi.fn(),
  listDatasets: vi.fn(),
  submitRagFeedback: vi.fn()
}));

const askDatasetMock = vi.mocked(askDataset);
const listDatasetsMock = vi.mocked(listDatasets);
const submitRagFeedbackMock = vi.mocked(submitRagFeedback);

function dataset(overrides: Partial<DatasetResp> = {}): DatasetResp {
  return {
    id: 10,
    tenantId: 1,
    name: "企业制度知识库",
    description: "政策、FAQ、流程",
    ownerId: 1,
    visibility: 1,
    status: 1,
    retrievalMode: 3,
    documentCount: 3,
    chunkCount: 28,
    createUserString: "admin",
    createTime: "2026-06-05 10:00:00",
    updateUserString: "",
    updateTime: "",
    ...overrides
  };
}

describe("Chat web page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listDatasetsMock.mockResolvedValue({
      list: [dataset()],
      total: 1
    });
    askDatasetMock.mockResolvedValue({
      traceId: 42,
      answer: "Use the current handbook.",
      citations: [
        {
          documentId: "20",
          chunkId: "20:0",
          pageNo: 3,
          sectionPath: ["Policy"]
        }
      ],
      retrievalHitCount: 1,
      answerStrategy: "extractive"
    });
    submitRagFeedbackMock.mockResolvedValue({
      id: 99,
      traceId: 42,
      rating: "citation_issue"
    });
  });

  it("renders a customer-facing knowledge chat workspace", async () => {
    render(<Page />);

    expect(screen.getByRole("heading", { name: "Novex Knowledge", level: 1 })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Sources" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Trace" })).toBeTruthy();
    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    expect((await screen.findAllByText("企业制度知识库")).length).toBeGreaterThan(0);
  });

  it("asks the selected dataset and renders citations with route metadata", async () => {
    render(<Page />);

    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    fireEvent.change(screen.getByLabelText("输入知识库问题"), {
      target: { value: "Which handbook should I use?" }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送问题" }));

    await waitFor(() =>
      expect(askDatasetMock).toHaveBeenCalledWith(10, {
        question: "Which handbook should I use?",
        limit: 5
      })
    );
    expect(await screen.findByText("Use the current handbook.")).toBeTruthy();
    expect(await screen.findByText("Trace #42")).toBeTruthy();
    expect(await screen.findByText("20:0 · page 3")).toBeTruthy();
    expect(await screen.findByText("Embedding local-keyword")).toBeTruthy();
  });

  it("submits citation feedback for the latest answer", async () => {
    render(<Page />);

    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    fireEvent.click(screen.getByRole("button", { name: "发送问题" }));
    expect(await screen.findByText("Use the current handbook.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "引用问题" }));

    await waitFor(() =>
      expect(submitRagFeedbackMock).toHaveBeenCalledWith({
        traceId: 42,
        rating: "citation_issue",
        reason: "chat-answer-feedback"
      })
    );
    expect(await screen.findByText("反馈已保存")).toBeTruthy();
  });
});
