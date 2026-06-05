"use client";

import { useEffect, useMemo, useState } from "react";
import {
  ArrowRight,
  Bell,
  Bot,
  CheckCircle2,
  CircleAlert,
  CircleDashed,
  Quote,
  Send,
  ShieldCheck,
  ThumbsDown,
  ThumbsUp
} from "lucide-react";
import { askDataset, listDatasets, submitRagFeedback } from "@/api/knowledge";
import { CitationList, type CitationItem } from "@/components/citation-list";
import { MetricStrip } from "@/components/metric-strip";
import { TrainingShell } from "@/components/training-shell";
import type { DatasetResp, RagAskResp, RagFeedbackRating } from "@/types/knowledge";

const metrics = [
  { label: "完成率", value: "68%", detail: "本周提升 12%", tone: "teal" as const },
  { label: "待学习", value: "4", detail: "2 项今天截止", tone: "amber" as const },
  { label: "测验均分", value: "86", detail: "高于团队均值", tone: "blue" as const },
  { label: "薄弱点", value: "3", detail: "集中在安全流程", tone: "rose" as const }
];

const tasks = [
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
];

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
  answerStrategy: "fixture"
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

export function TrainingAppClient() {
  const [datasets, setDatasets] = useState<DatasetResp[]>([fallbackDataset]);
  const [selectedDatasetId, setSelectedDatasetId] = useState(fallbackDataset.id);
  const [question, setQuestion] = useState("培训什么时候开始？");
  const [answer, setAnswer] = useState<RagAskResp>(fallbackAnswer);
  const [apiStatus, setApiStatus] = useState("fallback");
  const [asking, setAsking] = useState(false);
  const [feedbackSubmitting, setFeedbackSubmitting] = useState(false);
  const [feedbackStatus, setFeedbackStatus] = useState("");

  useEffect(() => {
    let mounted = true;

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
  }, []);

  const selectedDataset = useMemo(
    () => datasets.find((dataset) => dataset.id === selectedDatasetId) ?? fallbackDataset,
    [datasets, selectedDatasetId]
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
                <span className="inline-flex items-center gap-2 rounded-md bg-teal-50 px-3 py-2 text-sm font-medium text-teal-800 ring-1 ring-teal-100">
                  <ShieldCheck aria-hidden="true" className="h-4 w-4" />
                  已接入 RBAC
                </span>
                <span className="inline-flex items-center gap-2 rounded-md bg-slate-100 px-3 py-2 text-sm font-medium text-slate-700">
                  <Bot aria-hidden="true" className="h-4 w-4" />
                  {apiStatus === "live" ? "Live RAG" : "Fallback"}
                </span>
              </div>
            </div>
          </header>

          <MetricStrip metrics={metrics} />

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
                {tasks.map((task, index) => (
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
              <div className="rounded-lg bg-rose-50 p-3 text-rose-800">
                最近错题集中在客户数据外发和权限申请。
              </div>
            </div>
          </section>

          <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
            <div className="flex items-center gap-2">
              <Bell aria-hidden="true" className="h-4 w-4 text-teal-700" />
              <h2 className="text-sm font-semibold text-slate-950">通知状态</h2>
            </div>
            <div className="mt-3 text-sm leading-6 text-slate-600">
              飞书学习任务已发送，员工可从通知回到培训工作台继续学习。
            </div>
          </section>
        </aside>
      </div>
    </TrainingShell>
  );
}
