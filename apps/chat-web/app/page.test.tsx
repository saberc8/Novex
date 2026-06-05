import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { metadata } from "./layout";
import Page from "./page";
import { askDataset, listDatasets, submitRagFeedback } from "@/api/knowledge";
import { chatCompletion } from "@/api/model";
import type { DatasetResp } from "@/types/knowledge";

vi.mock("@/api/knowledge", () => ({
  askDataset: vi.fn(),
  listDatasets: vi.fn(),
  submitRagFeedback: vi.fn()
}));

vi.mock("@/api/model", () => ({
  chatCompletion: vi.fn()
}));

const askDatasetMock = vi.mocked(askDataset);
const listDatasetsMock = vi.mocked(listDatasets);
const submitRagFeedbackMock = vi.mocked(submitRagFeedback);
const chatCompletionMock = vi.mocked(chatCompletion);

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
    chatCompletionMock.mockResolvedValue({
      answer: "Pure model answer.",
      routeId: "runtime.llm",
      model: "deepseek-v4-flash",
      latencyMs: 42,
      usage: {
        promptTokens: 11,
        completionTokens: 7,
        totalTokens: 18
      }
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

  it("uses shared chat template metadata", () => {
    expect(metadata.title).toBe("Novex Chat");
    expect(metadata.description).toContain("model and knowledge");
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

  it("runs pure model chat from the customer-facing workspace", async () => {
    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "模型对话" }));
    fireEvent.change(screen.getByLabelText("输入模型问题"), {
      target: { value: "Explain Novex." }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送模型消息" }));

    await waitFor(() =>
      expect(chatCompletionMock).toHaveBeenCalledWith({
        messages: [{ role: "user", content: "Explain Novex." }],
        temperature: 0.2,
        maxTokens: 1024
      })
    );
    expect(await screen.findByText("Pure model answer.")).toBeTruthy();
    expect((await screen.findAllByText("runtime.llm · deepseek-v4-flash")).length).toBeGreaterThan(0);
    expect((await screen.findAllByText("42 ms")).length).toBeGreaterThan(0);
  });
});
