import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiEvalsPage from "./page";
import {
  listEvalCases,
  listEvalDatasets,
  listEvalResults,
  listEvalRuns,
  runEvalDataset
} from "@/api/ai/eval";
import { listTrainingLearningRecords } from "@/api/ai/training";
import type { EvalCaseResp, EvalDatasetResp, EvalResultResp, EvalRunResp } from "@/types/ai-eval";

vi.mock("@/api/ai/eval", () => ({
  listEvalCases: vi.fn(),
  listEvalDatasets: vi.fn(),
  listEvalResults: vi.fn(),
  listEvalRuns: vi.fn(),
  runEvalDataset: vi.fn()
}));

vi.mock("@/api/ai/training", () => ({
  listTrainingLearningRecords: vi.fn()
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

const listEvalCasesMock = vi.mocked(listEvalCases);
const listEvalDatasetsMock = vi.mocked(listEvalDatasets);
const listEvalResultsMock = vi.mocked(listEvalResults);
const listEvalRunsMock = vi.mocked(listEvalRuns);
const runEvalDatasetMock = vi.mocked(runEvalDataset);
const listTrainingLearningRecordsMock = vi.mocked(listTrainingLearningRecords);

function dataset(overrides: Partial<EvalDatasetResp> = {}): EvalDatasetResp {
  return {
    id: 700,
    code: "training_regression",
    name: "Training Regression",
    description: "Training regression smoke set",
    targetScope: "training",
    status: 1,
    metadata: {},
    caseCount: 20,
    createTime: "2026-06-05 10:00:00",
    ...overrides
  };
}

function evalCase(overrides: Partial<EvalCaseResp> = {}): EvalCaseResp {
  return {
    id: 710,
    datasetId: 700,
    caseCode: "rag-answer",
    targetKind: "rag",
    metricKind: "citation_accuracy",
    prompt: "培训什么时候开始？",
    expectedPayload: {
      answerContains: ["Monday"],
      citations: ["20:0"]
    },
    tags: ["training"],
    status: 1,
    sort: 1,
    createTime: "2026-06-05 10:00:00",
    ...overrides
  };
}

function run(overrides: Partial<EvalRunResp> = {}): EvalRunResp {
  return {
    runId: 810,
    datasetId: 700,
    datasetCode: "training_regression",
    status: "succeeded",
    totalCases: 20,
    passedCases: 18,
    failedCases: 2,
    averageScore: 0.9,
    metricBreakdown: {
      citation_accuracy: 0.9,
      intent_accuracy: 1,
      tool_accuracy: 0.8
    },
    reportPayload: {
      totalCases: 20,
      passedCases: 18,
      failedCases: 2,
      averageScore: 0.9,
      totalLatencyMs: 42,
      totalCostCents: 3
    },
    createTime: "2026-06-05 11:00:00",
    finishedAt: "2026-06-05 11:00:01",
    ...overrides
  };
}

function result(overrides: Partial<EvalResultResp> = {}): EvalResultResp {
  return {
    id: 910,
    runId: 810,
    caseId: 710,
    caseCode: "rag-answer",
    targetKind: "rag",
    metricKind: "citation_accuracy",
    score: 1,
    passed: true,
    expectedPayload: {},
    actualPayload: {
      answer: "Training starts on Monday.",
      citations: ["20:0"]
    },
    reason: "answer and citation matched",
    costCents: 1,
    latencyMs: 12,
    createTime: "2026-06-05 11:00:01",
    ...overrides
  };
}

describe("AiEvalsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listEvalDatasetsMock.mockResolvedValue({
      list: [dataset()],
      total: 1
    });
    listEvalCasesMock.mockResolvedValue({
      list: [
        evalCase(),
        evalCase({
          id: 711,
          caseCode: "intent-route",
          targetKind: "intent",
          metricKind: "intent_accuracy",
          prompt: "我要生成测验"
        }),
        evalCase({
          id: 712,
          caseCode: "tool-reminder",
          targetKind: "tool",
          metricKind: "tool_accuracy",
          prompt: "发送学习提醒"
        })
      ],
      total: 20
    });
    listEvalRunsMock.mockResolvedValue({
      list: [run()],
      total: 1
    });
    listEvalResultsMock.mockImplementation((runId: number) =>
      Promise.resolve({
        list: [result({ runId })],
        total: 1
      })
    );
    runEvalDatasetMock.mockResolvedValue(
      run({
        runId: 900,
        reportPayload: {
          totalCases: 20,
          passedCases: 20,
          failedCases: 0,
          averageScore: 1,
          totalLatencyMs: 18,
          totalCostCents: 2
        }
      })
    );
    listTrainingLearningRecordsMock.mockResolvedValue({
      scope: "tenant",
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
          learnerId: 9,
          learnerName: "Alice",
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

  it("loads eval datasets, cases, runs, results, and regression report payload", async () => {
    render(<AiEvalsPage />);

    expect(await screen.findByText("Training Regression")).toBeTruthy();
    await waitFor(() =>
      expect(listEvalCasesMock).toHaveBeenCalledWith(700, {
        page: 1,
        size: 100,
        targetKind: undefined
      })
    );
    await waitFor(() =>
      expect(listEvalRunsMock).toHaveBeenCalledWith({
        page: 1,
        size: 10,
        datasetCode: "training_regression"
      })
    );
    await waitFor(() => expect(listEvalResultsMock).toHaveBeenCalledWith(810, { page: 1, size: 100 }));

    expect(await screen.findByText("Run #810")).toBeTruthy();
    expect(screen.getByText("RAG Citation")).toBeTruthy();
    expect(screen.getByText("Retrieval Recall")).toBeTruthy();
    expect(screen.getByText("Intent Accuracy")).toBeTruthy();
    expect(screen.getByText("Tool Accuracy")).toBeTruthy();
    expect(screen.getByText("Latency")).toBeTruthy();
    expect(screen.getByText("Cost")).toBeTruthy();
    expect(screen.getAllByText("rag-answer").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("answer and citation matched")).toBeTruthy();
    expect(screen.getByText("Regression Payload")).toBeTruthy();
    expect(screen.getByText(/totalLatencyMs/)).toBeTruthy();
    expect(screen.getByText(/totalCostCents/)).toBeTruthy();
    await waitFor(() => expect(listTrainingLearningRecordsMock).toHaveBeenCalledWith({ scope: "tenant" }));
    expect(await screen.findByText("Training Learning Records")).toBeTruthy();
    expect(await screen.findByText("Alice")).toBeTruthy();
    expect(await screen.findByText("客户数据外发与权限申请")).toBeTruthy();
  });

  it("runs the selected eval dataset and reloads the report results", async () => {
    listEvalRunsMock
      .mockResolvedValueOnce({
        list: [run()],
        total: 1
      })
      .mockResolvedValueOnce({
        list: [run({ runId: 900 }), run()],
        total: 2
      });

    render(<AiEvalsPage />);

    expect(await screen.findByText("Training Regression")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Run Dataset" }));

    await waitFor(() =>
      expect(runEvalDatasetMock).toHaveBeenCalledWith({
        datasetId: 700,
        datasetCode: "training_regression"
      })
    );
    await waitFor(() => expect(listEvalResultsMock).toHaveBeenCalledWith(900, { page: 1, size: 100 }));
    expect(await screen.findByText("Run #900")).toBeTruthy();
  });
});
