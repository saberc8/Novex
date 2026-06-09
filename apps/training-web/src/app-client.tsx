"use client";

import { type ChangeEvent, type FormEvent, useEffect, useMemo, useState } from "react";
import {
  ArrowRight,
  BarChart3,
  Bell,
  Bot,
  CheckCircle2,
  CircleAlert,
  CircleDashed,
  LogIn,
  ListChecks,
  Quote,
  RotateCw,
  Send,
  ShieldCheck,
  ThumbsDown,
  ThumbsUp,
  Upload
} from "lucide-react";
import { accountLogin, getImageCaptcha } from "@/api/auth";
import { createAgentRun } from "@/api/agent";
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
import { CitationList, type CitationItem } from "@/components/citation-list";
import { MetricStrip } from "@/components/metric-strip";
import { TrainingShell } from "@/components/training-shell";
import { getAuthToken, setAuthToken as persistAuthToken } from "@/lib/auth";
import type { AgentRunResp } from "@/types/agent";
import type { ImageCaptchaResp } from "@/types/auth";
import type { EvalDatasetResp, EvalResultResp, EvalRunResp } from "@/types/eval";
import type { DatasetResp, ParserJobResp, RagAskResp, RagFeedbackRating } from "@/types/knowledge";
import type { TrainingLearningRecordsResp } from "@/types/training";

const TRAINING_CLIENT_ID = "novex-training-web";
const DOCUMENT_PARSE_STATUS_PARSED = 3;
const DOCUMENT_PARSE_STATUS_FAILED = 4;
const DOCUMENT_INGESTION_STATUS_INDEXED = 4;
const PARSE_JOB_POLL_INTERVAL_MS = 1500;
const MAX_PARSE_JOB_POLLS = 20;

const fallbackDataset: DatasetResp = {
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
  updateTime: ""
};

const fallbackAnswer: RagAskResp = {
  traceId: 0,
  answer: "不能。客户数据必须在受控系统内处理，外发需要审批并保留审计记录。",
  citations: [
    {
      documentId: "20",
      chunkId: "20:0",
      pageNo: null,
      sectionPath: ["信息安全入职手册"]
    },
    {
      documentId: "21",
      chunkId: "21:3",
      pageNo: null,
      sectionPath: ["客户数据处理规范"]
    }
  ],
  retrievalHitCount: 2,
  answerStrategy: "fixture",
  embeddingModelRoute: "local-keyword",
  rerankModelRoute: "none",
  answerModelRoute: "local-extractive",
  answerModel: null
};

const fallbackCitations: CitationItem[] = [
  {
    title: "信息安全入职手册",
    chunkId: "20:0",
    excerpt: "新员工必须在入职第一周完成账号安全、数据分级和外部协作规范培训。",
    score: "score 0.82"
  },
  {
    title: "客户数据处理规范",
    chunkId: "21:3",
    excerpt: "涉及客户身份、合同、工单和财务数据时，应使用受控系统并保留访问审计。",
    score: "score 0.76"
  }
];

const fallbackEvalDataset: EvalDatasetResp = {
  id: 700,
  code: "training_regression",
  name: "Training Regression",
  description: "Training regression smoke set",
  targetScope: "training",
  status: 1,
  metadata: {},
  caseCount: 20,
  createTime: "2026-06-05 10:00:00"
};

const fallbackLearningRecords: TrainingLearningRecordsResp = {
  scope: "self",
  summary: {
    completionRate: 68,
    pendingTaskCount: 4,
    quizAverageScore: 86,
    weakPointCount: 3
  },
  tasks: [
    {
      title: "完成信息安全入职培训",
      source: "入职制度知识库",
      due: "今日 18:00",
      status: "进行中"
    },
    {
      title: "阅读客户数据处理规范",
      source: "合规资料库",
      due: "明日 12:00",
      status: "未开始"
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
      id: 0,
      kind: "quiz_feedback",
      title: "测验错题反馈",
      detail: "客户数据外发和权限申请",
      status: "needs_review",
      score: null,
      learnerId: 1,
      learnerName: "admin",
      createTime: "2026-06-05 10:00:00"
    }
  ],
  weakPoints: [
    {
      topic: "客户数据外发与权限申请",
      evidence: "fallback",
      count: 2,
      lastSeenAt: "2026-06-05 10:00:00"
    }
  ]
};

function isParseJobIndexed(job: ParserJobResp) {
  return job.parseStatus === DOCUMENT_PARSE_STATUS_PARSED && job.ingestionStatus === DOCUMENT_INGESTION_STATUS_INDEXED;
}

function isParseJobFailed(job: ParserJobResp) {
  return job.parseStatus === DOCUMENT_PARSE_STATUS_FAILED;
}

function parseJobUploadStatus(job: ParserJobResp) {
  if (isParseJobIndexed(job)) {
    return `${job.documentName} 已解析并索引 ${job.chunkCount} 个片段，可提问`;
  }
  if (isParseJobFailed(job)) {
    return job.errorMessage ? `解析任务 #${job.id} 失败：${job.errorMessage}` : `解析任务 #${job.id} 失败`;
  }
  if (job.parseStatus === DOCUMENT_PARSE_STATUS_PARSED) {
    return `解析任务 #${job.id} 已解析，等待索引`;
  }
  return `解析任务 #${job.id} 已创建，等待解析`;
}

async function pollParseJob(datasetId: number, jobId: number) {
  let latest: ParserJobResp | null = null;
  for (let index = 0; index < MAX_PARSE_JOB_POLLS; index += 1) {
    latest = await getParseJob(datasetId, jobId);
    if (isParseJobIndexed(latest) || isParseJobFailed(latest)) {
      return latest;
    }
    await delay(PARSE_JOB_POLL_INTERVAL_MS);
  }
  return latest;
}

function delay(ms: number) {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

export function TrainingAppClient() {
  const [authToken, setAuthToken] = useState<string | null>(() => getAuthToken());
  const [loginUsername, setLoginUsername] = useState("admin");
  const [loginPassword, setLoginPassword] = useState("admin123");
  const [loginCaptcha, setLoginCaptcha] = useState("");
  const [captcha, setCaptcha] = useState<ImageCaptchaResp | null>(null);
  const [loggingIn, setLoggingIn] = useState(false);
  const [loginStatus, setLoginStatus] = useState("");
  const [datasets, setDatasets] = useState<DatasetResp[]>([fallbackDataset]);
  const [selectedDatasetId, setSelectedDatasetId] = useState(fallbackDataset.id);
  const [evalDatasets, setEvalDatasets] = useState<EvalDatasetResp[]>([fallbackEvalDataset]);
  const [evalRun, setEvalRun] = useState<EvalRunResp | null>(null);
  const [evalRuns, setEvalRuns] = useState<EvalRunResp[]>([]);
  const [evalResults, setEvalResults] = useState<EvalResultResp[]>([]);
  const [learningRecords, setLearningRecords] = useState<TrainingLearningRecordsResp>(fallbackLearningRecords);
  const [learningStatus, setLearningStatus] = useState("fallback");
  const [question, setQuestion] = useState("培训什么时候开始？");
  const [answer, setAnswer] = useState<RagAskResp>(fallbackAnswer);
  const [apiStatus, setApiStatus] = useState("fallback");
  const [evalStatus, setEvalStatus] = useState("fallback");
  const [asking, setAsking] = useState(false);
  const [evalRunning, setEvalRunning] = useState(false);
  const [feedbackSubmitting, setFeedbackSubmitting] = useState(false);
  const [feedbackStatus, setFeedbackStatus] = useState("");
  const [uploadingFile, setUploadingFile] = useState(false);
  const [uploadStatus, setUploadStatus] = useState("");
  const [quizRun, setQuizRun] = useState<AgentRunResp | null>(null);
  const [quizRunning, setQuizRunning] = useState(false);
  const [quizStatus, setQuizStatus] = useState("");
  const [quizFeedbackSubmitting, setQuizFeedbackSubmitting] = useState(false);
  const [quizFeedbackStatus, setQuizFeedbackStatus] = useState("");
  const [reminderRun, setReminderRun] = useState<AgentRunResp | null>(null);
  const [reminderSending, setReminderSending] = useState(false);
  const [reminderStatus, setReminderStatus] = useState("");

  useEffect(() => {
    let mounted = true;
    if (!authToken) {
      return () => {
        mounted = false;
      };
    }

    listDatasets({ page: 1, size: 20 })
      .then((page) => {
        if (!mounted) {
          return;
        }
        const nextDatasets = page.list.length > 0 ? page.list : [fallbackDataset];
        const preferred = nextDatasets.find((dataset) => dataset.documentCount > 0) ?? nextDatasets[0];
        setDatasets(nextDatasets);
        setSelectedDatasetId(preferred.id);
        setApiStatus("live");
      })
      .catch(() => {
        if (mounted) {
          setApiStatus("fallback");
        }
      });

    return () => {
      mounted = false;
    };
  }, [authToken]);

  useEffect(() => {
    let mounted = true;
    if (!authToken) {
      return () => {
        mounted = false;
      };
    }

    listEvalRuns({ page: 1, size: 5, datasetCode: "training_regression" })
      .then(async (page) => {
        if (!mounted) {
          return;
        }
        setEvalRuns(page.list);
        setEvalRun((current) => current ?? page.list[0] ?? null);
        const firstRun = page.list[0];
        if (firstRun) {
          await loadEvalResults(firstRun.runId, () => mounted);
        }
        setEvalStatus("live");
      })
      .catch(() => {
        if (mounted) {
          setEvalStatus("fallback");
        }
      });

    return () => {
      mounted = false;
    };
  }, [authToken]);

  useEffect(() => {
    let mounted = true;
    if (!authToken) {
      return () => {
        mounted = false;
      };
    }

    listEvalDatasets({ page: 1, size: 20, code: "training_regression" })
      .then((page) => {
        if (!mounted) {
          return;
        }
        setEvalDatasets(page.list.length > 0 ? page.list : [fallbackEvalDataset]);
        setEvalStatus("live");
      })
      .catch(() => {
        if (mounted) {
          setEvalStatus("fallback");
        }
      });

    return () => {
      mounted = false;
    };
  }, [authToken]);

  useEffect(() => {
    let mounted = true;
    if (!authToken) {
      return () => {
        mounted = false;
      };
    }

    listTrainingLearningRecords({ scope: "self" })
      .then((records) => {
        if (!mounted) {
          return;
        }
        setLearningRecords(records);
        setLearningStatus("live");
      })
      .catch(() => {
        if (mounted) {
          setLearningRecords(fallbackLearningRecords);
          setLearningStatus("fallback");
        }
      });

    return () => {
      mounted = false;
    };
  }, [authToken]);

  useEffect(() => {
    let mounted = true;
    if (authToken) {
      return () => {
        mounted = false;
      };
    }

    getImageCaptcha()
      .then((response) => {
        if (mounted) {
          setCaptcha(response);
        }
      })
      .catch(() => {
        if (mounted) {
          setCaptcha({ isEnabled: false, uuid: "", img: "" });
        }
      });

    return () => {
      mounted = false;
    };
  }, [authToken]);

  const selectedDataset = useMemo(
    () => datasets.find((dataset) => dataset.id === selectedDatasetId) ?? fallbackDataset,
    [datasets, selectedDatasetId]
  );

  const selectedEvalDataset = evalDatasets[0] ?? fallbackEvalDataset;
  const learningTasks =
    learningRecords.tasks.length > 0 ? learningRecords.tasks : fallbackLearningRecords.tasks;
  const learningMetrics = useMemo(
    () => [
      {
        label: "完成率",
        value: `${learningRecords.summary.completionRate}%`,
        detail: learningStatus === "live" ? `${learningRecords.records.length} 条学习记录` : "Fallback",
        tone: "teal" as const
      },
      {
        label: "待学习",
        value: String(learningRecords.summary.pendingTaskCount),
        detail: "来自任务状态",
        tone: "amber" as const
      },
      {
        label: "测验均分",
        value: String(learningRecords.summary.quizAverageScore),
        detail: "Training Regression",
        tone: "blue" as const
      },
      {
        label: "薄弱点",
        value: String(learningRecords.summary.weakPointCount),
        detail: learningRecords.weakPoints[0]?.topic ?? "暂无薄弱点",
        tone: "rose" as const
      }
    ],
    [learningRecords, learningStatus]
  );

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const username = loginUsername.trim();
    const password = loginPassword;
    if (!username || !password || loggingIn) {
      return;
    }

    setLoggingIn(true);
    setLoginStatus("");
    try {
      const response = await accountLogin({
        username,
        password,
        authType: "ACCOUNT",
        clientId: TRAINING_CLIENT_ID,
        ...(captcha?.isEnabled
          ? {
              captcha: loginCaptcha.trim(),
              uuid: captcha.uuid
            }
          : {})
      });
      persistAuthToken(response.token);
      setAuthToken(response.token);
      setLoginPassword("");
      setLoginCaptcha("");
    } catch {
      setLoginStatus("登录失败");
    } finally {
      setLoggingIn(false);
    }
  }

  const loginPanel = (
    <main className="min-h-screen bg-slate-100 text-slate-950">
      <div className="mx-auto flex min-h-screen max-w-[1440px] items-center justify-center p-4">
        <section className="w-full max-w-sm rounded-lg border border-slate-200 bg-white p-5 shadow-sm">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-teal-700 text-white">
              <LogIn aria-hidden="true" className="h-5 w-5" />
            </div>
            <div className="min-w-0">
              <h1 className="text-lg font-semibold text-slate-950">培训工作台登录</h1>
              <div className="mt-1 text-xs text-slate-500">Novex Training App</div>
            </div>
          </div>
          <form className="mt-5 space-y-3" onSubmit={(event) => void handleLogin(event)}>
            <label className="block text-sm font-medium text-slate-700">
              账号
              <input
                aria-label="账号"
                autoComplete="username"
                className="mt-1 h-10 w-full rounded-lg border border-slate-200 px-3 text-sm outline-none focus:border-teal-500"
                onChange={(event) => setLoginUsername(event.target.value)}
                value={loginUsername}
              />
            </label>
            <label className="block text-sm font-medium text-slate-700">
              密码
              <input
                aria-label="密码"
                autoComplete="current-password"
                className="mt-1 h-10 w-full rounded-lg border border-slate-200 px-3 text-sm outline-none focus:border-teal-500"
                onChange={(event) => setLoginPassword(event.target.value)}
                type="password"
                value={loginPassword}
              />
            </label>
            {captcha?.isEnabled ? (
              <div className="grid grid-cols-[minmax(0,1fr)_120px] gap-2">
                <label className="block text-sm font-medium text-slate-700">
                  验证码
                  <input
                    aria-label="验证码"
                    autoComplete="off"
                    className="mt-1 h-10 w-full rounded-lg border border-slate-200 px-3 text-sm outline-none focus:border-teal-500"
                    onChange={(event) => setLoginCaptcha(event.target.value)}
                    value={loginCaptcha}
                  />
                </label>
                <img
                  alt="验证码"
                  className="mt-6 h-10 rounded-lg border border-slate-200 object-cover"
                  src={captcha.img}
                />
              </div>
            ) : null}
            <button
              className="inline-flex h-10 w-full items-center justify-center rounded-lg bg-teal-700 px-4 text-sm font-semibold text-white hover:bg-teal-800 disabled:bg-slate-300"
              disabled={loggingIn}
              type="submit"
            >
              登录
            </button>
            {loginStatus ? (
              <div className="rounded-md bg-rose-50 px-3 py-2 text-sm font-medium text-rose-700">
                {loginStatus}
              </div>
            ) : null}
          </form>
        </section>
      </div>
    </main>
  );

  const citations = useMemo(() => {
    if (answer.traceId === 0) {
      return fallbackCitations;
    }

    return answer.citations.map((citation) => ({
      title: citation.sectionPath[0] || `Document ${citation.documentId}`,
      chunkId: citation.chunkId,
      excerpt: `Document ${citation.documentId} · ${citation.sectionPath.join(" / ") || "引用片段"}`,
      score: `trace ${answer.traceId}`
    }));
  }, [answer]);

  if (!authToken) {
    return loginPanel;
  }

  async function handleAsk() {
    const trimmed = question.trim();
    if (!trimmed || asking) {
      return;
    }

    setAsking(true);
    try {
      const response = await askDataset(selectedDataset.id, {
        question: trimmed,
        limit: 5
      });
      setAnswer(response);
      setApiStatus("live");
      setFeedbackStatus("");
    } catch {
      setApiStatus("fallback");
    } finally {
      setAsking(false);
    }
  }

  async function handleFeedback(rating: RagFeedbackRating) {
    if (answer.traceId <= 0 || feedbackSubmitting) {
      return;
    }

    setFeedbackSubmitting(true);
    setFeedbackStatus("");
    try {
      await submitRagFeedback({
        traceId: answer.traceId,
        rating,
        reason: "training-answer-feedback"
      });
      setFeedbackStatus("已记录反馈");
    } catch {
      setFeedbackStatus("反馈提交失败");
    } finally {
      setFeedbackSubmitting(false);
    }
  }

  async function refreshKnowledgeDatasets(preferredDatasetId: number) {
    const page = await listDatasets({ page: 1, size: 20 });
    const nextDatasets = page.list.length > 0 ? page.list : [fallbackDataset];
    const preferred =
      nextDatasets.find((dataset) => dataset.id === preferredDatasetId) ??
      nextDatasets.find((dataset) => dataset.documentCount > 0) ??
      nextDatasets[0];
    setDatasets(nextDatasets);
    setSelectedDatasetId(preferred.id);
  }

  async function handleTrainingFileUpload(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || uploadingFile) {
      return;
    }

    setUploadingFile(true);
    setUploadStatus("");
    try {
      const response = await uploadKnowledgeFile(selectedDataset.id, file);
      setUploadStatus(parseJobUploadStatus(response.parseJob));
      const finalJob = isParseJobIndexed(response.parseJob) || isParseJobFailed(response.parseJob)
        ? response.parseJob
        : await pollParseJob(selectedDataset.id, response.parseJob.id);
      if (finalJob) {
        setUploadStatus(parseJobUploadStatus(finalJob));
        if (isParseJobIndexed(finalJob)) {
          await refreshKnowledgeDatasets(selectedDataset.id);
        }
      }
      setApiStatus("live");
    } catch {
      setUploadStatus("上传失败");
    } finally {
      setUploadingFile(false);
    }
  }

  async function handleRunEval() {
    if (evalRunning) {
      return;
    }

    setEvalRunning(true);
    try {
      const response = await runEval({
        datasetCode: selectedEvalDataset.code
      });
      setEvalRun(response);
      setEvalRuns((current) => [response, ...current.filter((run) => run.runId !== response.runId)].slice(0, 5));
      await loadEvalResults(response.runId, () => true);
      setEvalStatus("live");
    } catch {
      setEvalStatus("fallback");
    } finally {
      setEvalRunning(false);
    }
  }

  async function loadEvalResults(runId: number, canUpdate: () => boolean) {
    try {
      const page = await listEvalResults(runId, { page: 1, size: 5 });
      if (canUpdate()) {
        setEvalResults(page.list);
      }
    } catch {
      if (canUpdate()) {
        setEvalResults([]);
      }
    }
  }

  async function handleGenerateQuiz() {
    if (quizRunning) {
      return;
    }

    setQuizRunning(true);
    setQuizStatus("");
    setQuizFeedbackStatus("");
    try {
      const response = await createAgentRun({
        input: "为信息安全入职培训生成 5 道测验题",
        autoApprove: true,
        budget: {
          maxSteps: 6,
          maxToolCalls: 0,
          maxSeconds: 30,
          maxCostCents: 0
        }
      });
      setQuizRun(response);
      setQuizStatus("测验已生成");
    } catch {
      setQuizStatus("测验生成失败");
    } finally {
      setQuizRunning(false);
    }
  }

  async function handleQuizFeedback() {
    if (!quizRun || quizFeedbackSubmitting) {
      return;
    }

    setQuizFeedbackSubmitting(true);
    setQuizFeedbackStatus("");
    try {
      await submitAiFeedback({
        resourceType: "training_quiz",
        resourceId: String(quizRun.runId),
        traceId: quizRun.traceId,
        rating: "quiz_wrong_answer",
        reason: "training-quiz-wrong-answer-feedback",
        metadata: {
          source: "training-web",
          quizRunId: quizRun.runId
        }
      });
      setQuizFeedbackStatus("错题反馈已记录");
    } catch {
      setQuizFeedbackStatus("错题反馈提交失败");
    } finally {
      setQuizFeedbackSubmitting(false);
    }
  }

  async function handleSendReminder() {
    if (reminderSending) {
      return;
    }

    setReminderSending(true);
    setReminderStatus("");
    try {
      const response = await createAgentRun({
        input: "发送飞书学习提醒：请完成信息安全入职培训",
        autoApprove: true,
        budget: {
          maxSteps: 6,
          maxToolCalls: 1,
          maxSeconds: 30,
          maxCostCents: 0
        }
      });
      setReminderRun(response);
      setReminderStatus(reminderStatusLabel(response.status));
    } catch {
      setReminderStatus("提醒发送失败");
    } finally {
      setReminderSending(false);
    }
  }

  return (
    <TrainingShell>
      <div className="grid min-w-0 grid-cols-1 gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_360px] lg:p-6">
        <section className="min-w-0 space-y-4">
          <header className="rounded-lg border border-slate-200 bg-white p-5 shadow-sm">
            <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
              <div>
                <div className="text-sm font-medium text-teal-700">Training App</div>
                <h1 className="mt-2 text-2xl font-semibold tracking-normal text-slate-950">
                  AI 员工培训
                </h1>
                <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-600">
                  今日重点是信息安全、客户数据处理和外部协作流程。
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                <label
                  className={[
                    "inline-flex h-9 cursor-pointer items-center gap-2 rounded-md border px-3 text-sm font-semibold",
                    uploadingFile
                      ? "border-slate-200 bg-slate-100 text-slate-400"
                      : "border-slate-200 text-slate-700 hover:bg-slate-50"
                  ].join(" ")}
                >
                  <Upload aria-hidden="true" className="h-4 w-4" />
                  上传资料
                  <input
                    aria-label="上传培训资料"
                    className="sr-only"
                    disabled={uploadingFile}
                    onChange={(event) => void handleTrainingFileUpload(event)}
                    type="file"
                  />
                </label>
                <span className="inline-flex items-center gap-2 rounded-md bg-teal-50 px-3 py-2 text-sm font-medium text-teal-800 ring-1 ring-teal-100">
                  <ShieldCheck aria-hidden="true" className="h-4 w-4" />
                  已接入 RBAC
                </span>
                <span className="inline-flex items-center gap-2 rounded-md bg-slate-100 px-3 py-2 text-sm font-medium text-slate-700">
                  <Bot aria-hidden="true" className="h-4 w-4" />
                  {apiStatus === "live" ? "Live RAG" : "Fallback"}
                </span>
                {uploadStatus ? (
                  <span className="inline-flex h-9 items-center rounded-md bg-slate-100 px-3 text-sm font-medium text-slate-700">
                    {uploadStatus}
                  </span>
                ) : null}
              </div>
            </div>
          </header>

          <MetricStrip metrics={learningMetrics} />

          <div className="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
            <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
              <div className="flex items-center justify-between gap-3">
                <h2 className="text-sm font-semibold text-slate-950">待学习任务</h2>
                <button
                  className="inline-flex items-center gap-1 rounded-md border border-slate-200 px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
                  type="button"
                >
                  全部任务
                  <ArrowRight aria-hidden="true" className="h-4 w-4" />
                </button>
              </div>
              <div className="mt-4 space-y-3">
                {learningTasks.map((task, index) => (
                  <article className="rounded-lg border border-slate-200 p-3" key={task.title}>
                    <div className="flex items-start gap-3">
                      {index === 0 ? (
                        <CircleDashed
                          aria-hidden="true"
                          className="mt-1 h-5 w-5 shrink-0 text-amber-600"
                        />
                      ) : (
                        <CheckCircle2
                          aria-hidden="true"
                          className="mt-1 h-5 w-5 shrink-0 text-slate-400"
                        />
                      )}
                      <div className="min-w-0 flex-1">
                        <div className="text-sm font-semibold text-slate-900">{task.title}</div>
                        <div className="mt-1 text-xs text-slate-500">
                          {task.source} · {task.due}
                        </div>
                      </div>
                      <span className="rounded-md bg-slate-100 px-2 py-1 text-xs text-slate-600">
                        {task.status}
                      </span>
                    </div>
                  </article>
                ))}
              </div>
            </section>

            <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
              <div className="flex items-center justify-between gap-3">
                <h2 className="text-sm font-semibold text-slate-950">知识库问答</h2>
                <span className="rounded-md bg-blue-50 px-2 py-1 text-xs font-medium text-blue-700">
                  {selectedDataset.name}
                </span>
              </div>
              <div className="mt-4 rounded-lg bg-slate-50 p-4">
                <div className="text-sm leading-6 text-slate-700">
                  问：客户数据能否复制到个人网盘？
                </div>
                <div className="mt-3 rounded-lg bg-white p-3 text-sm leading-6 text-slate-700">
                  {answer.answer}
                </div>
                <div className="mt-2 flex flex-wrap gap-1 text-xs text-slate-500">
                  <span>Trace #{answer.traceId}</span>
                  <span>·</span>
                  <span>{answer.retrievalHitCount} hits</span>
                  <span>·</span>
                  <span>{answer.answerStrategy}</span>
                </div>
                <div className="mt-3 flex flex-wrap items-center gap-2">
                  <button
                    aria-label="有帮助"
                    className="inline-flex h-8 items-center gap-1 rounded-md border border-slate-200 px-2 text-xs font-medium text-slate-600 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50"
                    disabled={answer.traceId <= 0 || feedbackSubmitting}
                    onClick={() => void handleFeedback("helpful")}
                    type="button"
                  >
                    <ThumbsUp aria-hidden="true" className="h-3.5 w-3.5" />
                    有帮助
                  </button>
                  <button
                    aria-label="答案不准确"
                    className="inline-flex h-8 items-center gap-1 rounded-md border border-slate-200 px-2 text-xs font-medium text-slate-600 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50"
                    disabled={answer.traceId <= 0 || feedbackSubmitting}
                    onClick={() => void handleFeedback("not_helpful")}
                    type="button"
                  >
                    <ThumbsDown aria-hidden="true" className="h-3.5 w-3.5" />
                    答案不准确
                  </button>
                  <button
                    aria-label="引用问题"
                    className="inline-flex h-8 items-center gap-1 rounded-md border border-slate-200 px-2 text-xs font-medium text-slate-600 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50"
                    disabled={answer.traceId <= 0 || feedbackSubmitting}
                    onClick={() => void handleFeedback("citation_issue")}
                    type="button"
                  >
                    <Quote aria-hidden="true" className="h-3.5 w-3.5" />
                    引用问题
                  </button>
                  {feedbackStatus ? (
                    <span className="inline-flex h-8 items-center gap-1 rounded-md bg-slate-100 px-2 text-xs font-medium text-slate-600">
                      <CircleAlert aria-hidden="true" className="h-3.5 w-3.5" />
                      {feedbackStatus}
                    </span>
                  ) : null}
                </div>
              </div>
              <div className="mt-4 flex gap-2">
                <input
                  aria-label="输入培训问题"
                  className="min-w-0 flex-1 rounded-lg border border-slate-200 px-3 py-2 text-sm outline-none focus:border-teal-500"
                  onChange={(event) => setQuestion(event.target.value)}
                  value={question}
                />
                <button
                  className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-teal-700 text-white hover:bg-teal-800 disabled:bg-slate-300"
                  type="button"
                  aria-label="发送问题"
                  disabled={asking}
                  onClick={handleAsk}
                >
                  <Send aria-hidden="true" className="h-4 w-4" />
                </button>
              </div>
            </section>
          </div>
        </section>

        <aside className="space-y-4">
          <CitationList citations={citations} />

          <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-2">
                <BarChart3 aria-hidden="true" className="h-4 w-4 text-teal-700" />
                <h2 className="text-sm font-semibold text-slate-950">质量回归</h2>
              </div>
              <span className="rounded-md bg-slate-100 px-2 py-1 text-xs font-medium text-slate-600">
                {evalStatus === "live" ? "Eval ready" : "Fallback"}
              </span>
            </div>
            <div className="mt-3 rounded-lg bg-slate-50 p-3">
              <div className="text-xs font-medium uppercase tracking-wide text-slate-500">
                {selectedEvalDataset.name}
              </div>
              <div className="mt-1 text-sm font-semibold text-slate-950">
                {selectedEvalDataset.code}
              </div>
              <div className="mt-1 text-xs text-slate-500">{selectedEvalDataset.caseCount} cases</div>
            </div>
            {evalRun ? (
              <div className="mt-3 grid grid-cols-2 gap-2">
                <div className="rounded-lg border border-slate-200 p-3">
                  <div className="text-xs text-slate-500">结果</div>
                  <div className="mt-1 text-sm font-semibold text-slate-950">
                    通过 {evalRun.passedCases} / {evalRun.totalCases}
                  </div>
                </div>
                <div className="rounded-lg border border-slate-200 p-3">
                  <div className="text-xs text-slate-500">均分</div>
                  <div className="mt-1 text-sm font-semibold text-slate-950">
                    平均 {evalRun.averageScore.toFixed(2)}
                  </div>
                </div>
              </div>
            ) : null}
            <button
              className="mt-3 inline-flex h-9 w-full items-center justify-center gap-2 rounded-md border border-slate-200 px-3 text-sm font-semibold text-slate-700 hover:bg-slate-50 disabled:text-slate-300"
              disabled={evalRunning}
              onClick={() => void handleRunEval()}
              type="button"
            >
              <RotateCw aria-hidden="true" className="h-4 w-4" />
              运行评测
            </button>
            <div className="mt-3 rounded-lg border border-slate-200 p-3">
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-sm font-semibold text-slate-950">最近回归</h3>
                {evalRun ? (
                  <span className="rounded-md bg-slate-100 px-2 py-1 text-xs font-medium text-slate-600">
                    Run #{evalRun.runId}
                  </span>
                ) : null}
              </div>
              {evalRun ? (
                <div className="mt-2 text-xs leading-5 text-slate-500">
                  {evalRun.status} · {evalRun.passedCases}/{evalRun.totalCases} passed · avg{" "}
                  {evalRun.averageScore.toFixed(2)}
                </div>
              ) : (
                <div className="mt-2 text-xs leading-5 text-slate-500">暂无回归记录</div>
              )}
              {evalRuns.length > 1 ? (
                <div className="mt-2 flex flex-wrap gap-1">
                  {evalRuns.slice(0, 3).map((run) => (
                    <button
                      className={[
                        "rounded-md border px-2 py-1 text-xs font-medium",
                        evalRun?.runId === run.runId
                          ? "border-teal-200 bg-teal-50 text-teal-900"
                          : "border-slate-200 text-slate-600 hover:bg-slate-50"
                      ].join(" ")}
                      key={run.runId}
                      onClick={() => {
                        setEvalRun(run);
                        void loadEvalResults(run.runId, () => true);
                      }}
                      type="button"
                    >
                      #{run.runId}
                    </button>
                  ))}
                </div>
              ) : null}
              <div className="mt-3 space-y-2">
                {evalResults.slice(0, 3).map((result) => (
                  <div
                    className="flex items-center justify-between gap-3 rounded-md bg-slate-50 px-3 py-2"
                    key={result.id}
                  >
                    <div className="min-w-0">
                      <div className="truncate text-xs font-semibold text-slate-900">
                        {result.caseCode}
                      </div>
                      <div className="mt-1 text-[11px] text-slate-500">
                        {result.metricKind} · score {result.score.toFixed(2)}
                      </div>
                    </div>
                    <span
                      className={[
                        "shrink-0 rounded-md px-2 py-1 text-xs font-medium",
                        result.passed ? "bg-emerald-50 text-emerald-700" : "bg-rose-50 text-rose-700"
                      ].join(" ")}
                    >
                      {result.passed ? "通过" : "未通过"}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </section>

          <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
            <div className="flex items-center justify-between gap-3">
              <h2 className="text-sm font-semibold text-slate-950">测验与错题</h2>
              <span className="rounded-md bg-amber-50 px-2 py-1 text-xs font-medium text-amber-700">
                5 题待完成
              </span>
            </div>
            <div className="mt-4 space-y-3 text-sm text-slate-600">
              <div className="flex items-center justify-between gap-3">
                <span>信息安全测验</span>
                <span className="font-medium text-slate-900">3/5</span>
              </div>
              <div className="h-2 overflow-hidden rounded-full bg-slate-100">
                <div className="h-full w-3/5 rounded-full bg-amber-500" />
              </div>
              <div className="space-y-2">
                {learningRecords.weakPoints.slice(0, 3).map((weakPoint) => (
                  <div className="rounded-lg bg-rose-50 p-3 text-rose-800" key={`${weakPoint.topic}-${weakPoint.evidence}`}>
                    <div className="font-medium">{weakPoint.topic}</div>
                    <div className="mt-1 text-xs text-rose-700">
                      {weakPoint.evidence} · {weakPoint.count} 次 · {weakPoint.lastSeenAt}
                    </div>
                  </div>
                ))}
                {!learningRecords.weakPoints.length ? (
                  <div className="rounded-lg bg-emerald-50 p-3 text-emerald-800">
                    暂无薄弱点记录。
                  </div>
                ) : null}
              </div>
              <button
                className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md border border-slate-200 px-3 text-sm font-semibold text-slate-700 hover:bg-slate-50 disabled:text-slate-300"
                disabled={quizRunning}
                onClick={() => void handleGenerateQuiz()}
                type="button"
              >
                <ListChecks aria-hidden="true" className="h-4 w-4" />
                生成测验
              </button>
              {quizStatus ? (
                <div className="rounded-lg border border-slate-200 p-3">
                  <div className="flex items-center justify-between gap-3">
                    <span className="text-sm font-medium text-slate-900">{quizStatus}</span>
                    {quizRun ? (
                      <span className="rounded-md bg-slate-100 px-2 py-1 text-xs font-medium text-slate-600">
                        Run #{quizRun.runId}
                      </span>
                    ) : null}
                  </div>
                  {quizRun?.finalOutput ? (
                    <div className="mt-2 text-xs leading-5 text-slate-500">
                      {quizRun.finalOutput}
                    </div>
                  ) : null}
                  {quizRun ? (
                    <div className="mt-3 flex flex-wrap items-center gap-2">
                      <button
                        className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-200 px-2 text-xs font-medium text-slate-600 hover:bg-slate-50 disabled:text-slate-300"
                        disabled={quizFeedbackSubmitting}
                        onClick={() => void handleQuizFeedback()}
                        type="button"
                      >
                        <CircleAlert aria-hidden="true" className="h-3.5 w-3.5" />
                        反馈错题
                      </button>
                      {quizFeedbackStatus ? (
                        <span className="inline-flex h-8 items-center rounded-md bg-slate-100 px-2 text-xs font-medium text-slate-600">
                          {quizFeedbackStatus}
                        </span>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              ) : null}
            </div>
          </section>

          <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
            <div className="flex items-center justify-between gap-3">
              <h2 className="text-sm font-semibold text-slate-950">学习记录</h2>
              <span className="rounded-md bg-slate-100 px-2 py-1 text-xs font-medium text-slate-600">
                {learningStatus === "live" ? "Live" : "Fallback"}
              </span>
            </div>
            <div className="mt-3 space-y-2">
              {learningRecords.records.slice(0, 4).map((record) => (
                <article className="rounded-lg border border-slate-200 p-3" key={`${record.kind}-${record.id}`}>
                  <div className="flex items-center justify-between gap-3">
                    <div className="min-w-0">
                      <div className="truncate text-sm font-semibold text-slate-900">{record.title}</div>
                      <div className="mt-1 truncate text-xs text-slate-500">
                        {record.learnerName} · {record.createTime}
                      </div>
                    </div>
                    <span className="shrink-0 rounded-md bg-slate-100 px-2 py-1 text-xs text-slate-600">
                      {record.status}
                    </span>
                  </div>
                  <div className="mt-2 line-clamp-2 text-xs leading-5 text-slate-500">
                    {record.detail}
                  </div>
                </article>
              ))}
              {!learningRecords.records.length ? (
                <div className="rounded-lg border border-dashed border-slate-200 p-4 text-center text-sm text-slate-500">
                  暂无学习记录
                </div>
              ) : null}
            </div>
          </section>

          <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-2">
                <Bell aria-hidden="true" className="h-4 w-4 text-teal-700" />
                <h2 className="text-sm font-semibold text-slate-950">通知状态</h2>
              </div>
              {reminderRun ? (
                <span className="rounded-md bg-slate-100 px-2 py-1 text-xs font-medium text-slate-600">
                  Run #{reminderRun.runId}
                </span>
              ) : null}
            </div>
            <div className="mt-3 text-sm leading-6 text-slate-600">
              {reminderStatus || "飞书学习任务已发送，员工可从通知回到培训工作台继续学习。"}
            </div>
            {reminderRun?.finalOutput ? (
              <div className="mt-2 rounded-lg bg-slate-50 p-3 text-xs leading-5 text-slate-500">
                {reminderRun.finalOutput}
              </div>
            ) : null}
            <button
              className="mt-3 inline-flex h-9 w-full items-center justify-center gap-2 rounded-md border border-slate-200 px-3 text-sm font-semibold text-slate-700 hover:bg-slate-50 disabled:text-slate-300"
              disabled={reminderSending}
              onClick={() => void handleSendReminder()}
              type="button"
            >
              <Bell aria-hidden="true" className="h-4 w-4" />
              发送学习提醒
            </button>
          </section>
        </aside>
      </div>
    </TrainingShell>
  );
}

function reminderStatusLabel(status: string) {
  switch (status) {
    case "succeeded":
      return "提醒已发送";
    case "waiting_approval":
    case "paused":
      return "等待管理员审批";
    case "cancelled":
      return "提醒已取消";
    case "failed":
      return "提醒发送失败";
    default:
      return "提醒处理中";
  }
}
