import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Page from "./page";
import { accountLogin, getImageCaptcha } from "@/api/auth";
import { createAgentRun } from "@/api/agent";
import { dryRunTool } from "@/api/capability";
import { listEvalDatasets, listEvalResults, listEvalRuns, runEval } from "@/api/eval";
import {
  askDataset,
  getParseJob,
  listDatasets,
  submitAiFeedback,
  submitRagFeedback,
  uploadKnowledgeFile
} from "@/api/knowledge";
import { listTrainingLearningRecords } from "@/api/training";
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
  getParseJob: vi.fn(),
  listDatasets: vi.fn(),
  submitAiFeedback: vi.fn(),
  submitRagFeedback: vi.fn(),
  uploadKnowledgeFile: vi.fn()
}));

vi.mock("@/api/training", () => ({
  listTrainingLearningRecords: vi.fn()
}));

vi.mock("next/navigation", () => ({
  usePathname: () => "/"
}));

const accountLoginMock = vi.mocked(accountLogin);
const askDatasetMock = vi.mocked(askDataset);
const createAgentRunMock = vi.mocked(createAgentRun);
const dryRunToolMock = vi.mocked(dryRunTool);
const getImageCaptchaMock = vi.mocked(getImageCaptcha);
const getParseJobMock = vi.mocked(getParseJob);
const listEvalDatasetsMock = vi.mocked(listEvalDatasets);
const listEvalResultsMock = vi.mocked(listEvalResults);
const listEvalRunsMock = vi.mocked(listEvalRuns);
const listDatasetsMock = vi.mocked(listDatasets);
const runEvalMock = vi.mocked(runEval);
const submitAiFeedbackMock = vi.mocked(submitAiFeedback);
const submitRagFeedbackMock = vi.mocked(submitRagFeedback);
const uploadKnowledgeFileMock = vi.mocked(uploadKnowledgeFile);
const listTrainingLearningRecordsMock = vi.mocked(listTrainingLearningRecords);

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
    submitAiFeedbackMock.mockResolvedValue({
      id: 100,
      resourceType: "training_quiz",
      resourceId: "900",
      traceId: "agent-900",
      rating: "quiz_wrong_answer"
    });
    uploadKnowledgeFileMock.mockResolvedValue({
      file: {
        id: 88,
        name: "88.md",
        originalName: "handbook.md",
        size: 24,
        url: "/file/knowledge/88.md",
        parentPath: "/knowledge",
        path: "/knowledge/88.md",
        sha256: "hash",
        contentType: "text/markdown",
        metadata: "{}",
        thumbnailSize: 0,
        thumbnailName: "",
        thumbnailMetadata: "",
        thumbnailUrl: "",
        extension: "md",
        type: 4,
        storageId: 1,
        storageName: "本地",
        createUserString: "admin",
        createTime: "2026-06-05 10:00:00",
        updateUserString: "",
        updateTime: ""
      },
      parseJob: {
        id: 99,
        tenantId: 1,
        datasetId: 10,
        documentId: 42,
        jobType: 2,
        status: 2,
        attemptCount: 0,
        errorMessage: "",
        resultSummary: {},
        documentName: "handbook.md",
        sourceUri: "/file/knowledge/88.md",
        fileId: 88,
        contentType: "text/markdown",
        parseStatus: 2,
        ingestionStatus: 1,
        chunkCount: 0,
        parserRequest: {},
        createUserString: "",
        createTime: "2026-06-05 10:00:00",
        updateUserString: "",
        updateTime: ""
      }
    });
    getParseJobMock.mockResolvedValue({
      id: 99,
      tenantId: 1,
      datasetId: 10,
      documentId: 42,
      jobType: 2,
      status: 3,
      attemptCount: 1,
      errorMessage: "",
      resultSummary: {},
      documentName: "handbook.md",
      sourceUri: "/file/knowledge/88.md",
      fileId: 88,
      contentType: "text/markdown",
      parseStatus: 3,
      ingestionStatus: 4,
      chunkCount: 3,
      parserRequest: {},
      createUserString: "",
      createTime: "2026-06-05 10:00:00",
      updateUserString: "",
      updateTime: ""
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
          caseCount: 20,
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
          totalCases: 20,
          passedCases: 18,
          failedCases: 2,
          averageScore: 0.9,
          metricBreakdown: { citation_accuracy: 0.9 },
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
      totalCases: 20,
      passedCases: 18,
      failedCases: 2,
      averageScore: 0.9,
      metricBreakdown: { citation_accuracy: 0.9 },
      reportPayload: {},
      createTime: "2026-06-05 12:00:00",
      finishedAt: "2026-06-05 12:00:01"
    });
    listTrainingLearningRecordsMock.mockResolvedValue({
      scope: "self",
      summary: {
        completionRate: 72,
        pendingTaskCount: 2,
        quizAverageScore: 91,
        weakPointCount: 2
      },
      tasks: [
        {
          title: "完成信息安全入职培训",
          source: "入职制度知识库",
          due: "今日 18:00",
          status: "进行中"
        },
        {
          title: "复盘本周错题",
          source: "测验记录",
          due: "周五前",
          status: "待复习"
        }
      ],
      records: [
        {
          id: 501,
          kind: "quiz_feedback",
          title: "测验错题反馈",
          detail: "客户数据外发",
          status: "needs_review",
          score: null,
          learnerId: 1,
          learnerName: "admin",
          createTime: "2026-06-05 12:10:00"
        }
      ],
      weakPoints: [
        {
          topic: "客户数据外发与权限申请",
          evidence: "quiz_wrong_answer",
          count: 2,
          lastSeenAt: "2026-06-05 12:10:00"
        }
      ]
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

  it("loads live learning records and weak points for the employee workbench", async () => {
    render(<Page />);

    await waitFor(() => expect(listTrainingLearningRecordsMock).toHaveBeenCalledWith({ scope: "self" }));
    expect(await screen.findByText("72%")).toBeTruthy();
    expect(await screen.findByText("91")).toBeTruthy();
    expect((await screen.findAllByText("客户数据外发与权限申请")).length).toBeGreaterThan(0);
    expect(await screen.findByText("测验错题反馈")).toBeTruthy();
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

  it("uploads training files and polls until the parsed document is ready for questions", async () => {
    render(<Page />);

    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 20 }));
    const file = new File(["# Handbook"], "handbook.md", { type: "text/markdown" });
    fireEvent.change(screen.getByLabelText("上传培训资料"), {
      target: { files: [file] }
    });

    await waitFor(() => expect(uploadKnowledgeFileMock).toHaveBeenCalledWith(10, file));
    await waitFor(() => expect(getParseJobMock).toHaveBeenCalledWith(10, 99));
    expect(await screen.findByText("handbook.md 已解析并索引 3 个片段，可提问")).toBeTruthy();
    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledTimes(2));
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
    expect(await screen.findByText("通过 18 / 20")).toBeTruthy();
    expect(await screen.findByText("平均 0.90")).toBeTruthy();
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

  it("submits quiz wrong-answer feedback from the customer workbench", async () => {
    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "生成测验" }));
    expect(await screen.findByText("测验已生成")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "反馈错题" }));

    await waitFor(() =>
      expect(submitAiFeedbackMock).toHaveBeenCalledWith({
        resourceType: "training_quiz",
        resourceId: "900",
        traceId: "agent-900",
        rating: "quiz_wrong_answer",
        reason: "training-quiz-wrong-answer-feedback",
        metadata: {
          source: "training-web",
          quizRunId: 900
        }
      })
    );
    expect(await screen.findByText("错题反馈已记录")).toBeTruthy();
  });

  it("runs a Feishu training reminder through the Agent tool loop", async () => {
    createAgentRunMock.mockResolvedValueOnce({
      runId: 901,
      traceId: "agent-901",
      status: "succeeded",
      intent: "tool_task",
      loopKind: "react",
      selectedToolCode: "feishu.message.send",
      pauseReason: null,
      finalOutput: "Feishu notification sent.",
      taskBudget: {
        maxSteps: 6,
        maxToolCalls: 1,
        maxSeconds: 30,
        maxCostCents: 0
      },
      createTime: "2026-06-05 12:20:00",
      updateTime: "2026-06-05 12:20:01"
    });
    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "发送学习提醒" }));

    await waitFor(() =>
      expect(createAgentRunMock).toHaveBeenCalledWith({
        input: "发送飞书学习提醒：请完成信息安全入职培训",
        autoApprove: true,
        budget: {
          maxSteps: 6,
          maxToolCalls: 1,
          maxSeconds: 30,
          maxCostCents: 0
        }
      })
    );
    expect(dryRunToolMock).not.toHaveBeenCalled();
    expect(await screen.findByText("提醒已发送")).toBeTruthy();
    expect(await screen.findByText("Run #901")).toBeTruthy();
    expect(screen.queryByText("permissionCode")).toBeNull();
  });

  it("shows a user-readable Feishu reminder approval state", async () => {
    createAgentRunMock.mockResolvedValueOnce({
      runId: 902,
      traceId: "agent-902",
      status: "waiting_approval",
      intent: "tool_task",
      loopKind: "react",
      selectedToolCode: "feishu.message.send",
      pauseReason: "approval",
      finalOutput: null,
      taskBudget: {
        maxSteps: 6,
        maxToolCalls: 1,
        maxSeconds: 30,
        maxCostCents: 0
      },
      createTime: "2026-06-05 12:25:00",
      updateTime: null
    });
    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "发送学习提醒" }));

    expect(await screen.findByText("等待管理员审批")).toBeTruthy();
    expect(screen.queryByText("waiting_approval")).toBeNull();
    expect(screen.queryByText("approval")).toBeNull();
  });
});
