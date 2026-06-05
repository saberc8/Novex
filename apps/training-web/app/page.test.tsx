import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Page from "./page";
import { accountLogin, getImageCaptcha } from "@/api/auth";
import { createAgentRun } from "@/api/agent";
import { dryRunTool } from "@/api/capability";
import { listEvalDatasets, listEvalResults, listEvalRuns, runEval } from "@/api/eval";
import { askDataset, listDatasets, submitRagFeedback } from "@/api/knowledge";
import type { DatasetResp } from "@/types/knowledge";

vi.mock("@/api/auth", () => ({
  accountLogin: vi.fn(),
  getImageCaptcha: vi.fn()
}));

vi.mock("@/api/agent", () => ({
  createAgentRun: vi.fn()
}));

vi.mock("@/api/capability", () => ({
  dryRunTool: vi.fn()
}));

vi.mock("@/api/eval", () => ({
  listEvalDatasets: vi.fn(),
  listEvalResults: vi.fn(),
  listEvalRuns: vi.fn(),
  runEval: vi.fn()
}));

vi.mock("@/api/knowledge", () => ({
  askDataset: vi.fn(),
  listDatasets: vi.fn(),
  submitRagFeedback: vi.fn()
}));

const accountLoginMock = vi.mocked(accountLogin);
const askDatasetMock = vi.mocked(askDataset);
const createAgentRunMock = vi.mocked(createAgentRun);
const dryRunToolMock = vi.mocked(dryRunTool);
const getImageCaptchaMock = vi.mocked(getImageCaptcha);
const listEvalDatasetsMock = vi.mocked(listEvalDatasets);
const listEvalResultsMock = vi.mocked(listEvalResults);
const listEvalRunsMock = vi.mocked(listEvalRuns);
const listDatasetsMock = vi.mocked(listDatasets);
const runEvalMock = vi.mocked(runEval);
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
    window.localStorage.clear();
    window.localStorage.setItem("novex_token", "token-1");
    getImageCaptchaMock.mockResolvedValue({
      isEnabled: false,
      uuid: "",
      img: ""
    });
    accountLoginMock.mockResolvedValue({
      token: "token-2",
      expire: "2099-01-01T00:00:00Z"
    });
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
    createAgentRunMock.mockResolvedValue({
      runId: 900,
      traceId: "agent-900",
      status: "succeeded",
      intent: "training_quiz",
      loopKind: "react",
      selectedToolCode: null,
      pauseReason: null,
      finalOutput: "测验已生成：请根据培训资料回答 5 道题。",
      taskBudget: {
        maxSteps: 6,
        maxToolCalls: 0,
        maxSeconds: 30,
        maxCostCents: 0
      },
      createTime: "2026-06-05 12:00:00",
      updateTime: "2026-06-05 12:00:01"
    });
    dryRunToolMock.mockResolvedValue({
      auditId: 901,
      toolCode: "feishu.message.send",
      status: "succeeded",
      dryRun: true,
      response: {
        message: "dry-run only; no external side effects"
      }
    });
    listEvalDatasetsMock.mockResolvedValue({
      list: [
        {
          id: 700,
          code: "training_regression",
          name: "Training Regression",
          description: "Training regression smoke set",
          targetScope: "training",
          status: 1,
          metadata: {},
          caseCount: 3,
          createTime: "2026-06-05 12:00:00"
        }
      ],
      total: 1
    });
    listEvalRunsMock.mockResolvedValue({
      list: [
        {
          runId: 810,
          datasetId: 700,
          datasetCode: "training_regression",
          status: "succeeded",
          totalCases: 3,
          passedCases: 2,
          failedCases: 1,
          averageScore: 0.67,
          metricBreakdown: { citation_accuracy: 0.67 },
          reportPayload: {},
          createTime: "2026-06-05 11:00:00",
          finishedAt: "2026-06-05 11:00:01"
        }
      ],
      total: 1
    });
    listEvalResultsMock.mockResolvedValue({
      list: [
        {
          id: 910,
          runId: 810,
          caseId: 710,
          caseCode: "rag-answer",
          targetKind: "rag",
          metricKind: "citation_accuracy",
          score: 1,
          passed: true,
          expectedPayload: {},
          actualPayload: {},
          reason: "matched answer and citations",
          costCents: 0,
          latencyMs: 12,
          createTime: "2026-06-05 11:00:01"
        }
      ],
      total: 1
    });
    runEvalMock.mockResolvedValue({
      runId: 800,
      datasetId: 700,
      datasetCode: "training_regression",
      status: "succeeded",
      totalCases: 3,
      passedCases: 2,
      failedCases: 1,
      averageScore: 0.67,
      metricBreakdown: { citation_accuracy: 0.67 },
      reportPayload: {},
      createTime: "2026-06-05 12:00:00",
      finishedAt: "2026-06-05 12:00:01"
    });
  });

  it("requires customer login before loading live training data", async () => {
    window.localStorage.clear();
    render(<Page />);

    expect(screen.getByRole("heading", { name: "培训工作台登录", level: 1 })).toBeTruthy();
    expect(listDatasetsMock).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText("账号"), {
      target: { value: "employee" }
    });
    fireEvent.change(screen.getByLabelText("密码"), {
      target: { value: "employee123" }
    });
    fireEvent.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() =>
      expect(accountLoginMock).toHaveBeenCalledWith({
        username: "employee",
        password: "employee123",
        authType: "ACCOUNT",
        clientId: "novex-training-web"
      })
    );
    expect(window.localStorage.getItem("novex_token")).toBe("token-2");
    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    expect(await screen.findByRole("heading", { name: "AI 员工培训", level: 1 })).toBeTruthy();
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

  it("runs the training regression eval set from the customer workbench", async () => {
    render(<Page />);

    await waitFor(() =>
      expect(listEvalDatasetsMock).toHaveBeenCalledWith({
        page: 1,
        size: 20,
        code: "training_regression"
      })
    );
    expect(await screen.findByText("training_regression")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "运行评测" }));

    await waitFor(() =>
      expect(runEvalMock).toHaveBeenCalledWith({
        datasetCode: "training_regression"
      })
    );
    expect(await screen.findByText("通过 2 / 3")).toBeTruthy();
    expect(await screen.findByText("平均 0.67")).toBeTruthy();
    await waitFor(() => expect(listEvalResultsMock).toHaveBeenCalledWith(800, { page: 1, size: 5 }));
    expect(await screen.findByText("Run #800")).toBeTruthy();
  });

  it("loads recent eval runs and shows case result snapshots", async () => {
    render(<Page />);

    await waitFor(() =>
      expect(listEvalRunsMock).toHaveBeenCalledWith({
        page: 1,
        size: 5,
        datasetCode: "training_regression"
      })
    );
    await waitFor(() => expect(listEvalResultsMock).toHaveBeenCalledWith(810, { page: 1, size: 5 }));
    expect(await screen.findByText("最近回归")).toBeTruthy();
    expect(await screen.findByText("Run #810")).toBeTruthy();
    expect(await screen.findByText("rag-answer")).toBeTruthy();
    expect(await screen.findByText("通过")).toBeTruthy();
  });

  it("runs the quiz skill from the customer workbench with a bounded agent budget", async () => {
    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "生成测验" }));

    await waitFor(() =>
      expect(createAgentRunMock).toHaveBeenCalledWith({
        input: "为信息安全入职培训生成 5 道测验题",
        autoApprove: true,
        budget: {
          maxSteps: 6,
          maxToolCalls: 0,
          maxSeconds: 30,
          maxCostCents: 0
        }
      })
    );
    expect(await screen.findByText("测验已生成")).toBeTruthy();
    expect(await screen.findByText("Run #900")).toBeTruthy();
    expect(screen.queryByText("ai:tool:dryRun")).toBeNull();
  });

  it("dry-runs a Feishu training reminder and shows the audit status to employees", async () => {
    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "发送学习提醒" }));

    await waitFor(() =>
      expect(dryRunToolMock).toHaveBeenCalledWith({
        toolCode: "feishu.message.send",
        input: {
          recipient: "training-team",
          text: "请完成信息安全入职培训"
        }
      })
    );
    expect(await screen.findByText("提醒已发送（演练）")).toBeTruthy();
    expect(await screen.findByText("Audit #901")).toBeTruthy();
    expect(screen.queryByText("permissionCode")).toBeNull();
  });
});
