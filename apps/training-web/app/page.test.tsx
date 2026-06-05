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
    name: "入职制度知识库",
    description: "培训资料",
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

describe("Training home page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listDatasetsMock.mockResolvedValue({
      list: [dataset()],
      total: 1
    });
    askDatasetMock.mockResolvedValue({
      traceId: 42,
      answer: "Live answer from RAG.",
      citations: [
        {
          documentId: "20",
          chunkId: "20:0",
          pageNo: null,
          sectionPath: ["入职"]
        }
      ],
      retrievalHitCount: 1,
      answerStrategy: "extractive"
    });
    submitRagFeedbackMock.mockResolvedValue({
      id: 99,
      traceId: 42,
      rating: "not_helpful"
    });
  });

  it("renders the customer-facing training workbench sections", () => {
    render(<Page />);

    expect(screen.getByRole("heading", { name: "AI 员工培训", level: 1 })).toBeTruthy();
    expect(screen.getByText("待学习任务")).toBeTruthy();
    expect(screen.getByText("知识库问答")).toBeTruthy();
    expect(screen.getByText("测验与错题")).toBeTruthy();
    expect(screen.getByText("引用来源")).toBeTruthy();
  });

  it("loads the first dataset and submits training questions to the RAG API", async () => {
    render(<Page />);

    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));

    fireEvent.change(screen.getByLabelText("输入培训问题"), {
      target: { value: "培训什么时候开始？" }
    });
    fireEvent.click(screen.getByLabelText("发送问题"));

    await waitFor(() =>
      expect(askDatasetMock).toHaveBeenCalledWith(10, {
        question: "培训什么时候开始？",
        limit: 5
      })
    );
    expect(await screen.findByText("Live answer from RAG.")).toBeTruthy();
    expect(await screen.findByText("20:0")).toBeTruthy();
    expect(await screen.findByText("Trace #42")).toBeTruthy();
  });

  it("submits RAG answer feedback for the latest live trace", async () => {
    render(<Page />);

    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    fireEvent.click(screen.getByLabelText("发送问题"));
    expect(await screen.findByText("Live answer from RAG.")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "答案不准确" }));

    await waitFor(() =>
      expect(submitRagFeedbackMock).toHaveBeenCalledWith({
        traceId: 42,
        rating: "not_helpful",
        reason: "training-answer-feedback"
      })
    );
    expect(await screen.findByText("已记录反馈")).toBeTruthy();
  });
});
