"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowRight,
  Bot,
  Check,
  ChevronRight,
  Database,
  FileText,
  LayoutGrid,
  MoreVertical,
  PanelLeft,
  Plus,
  Quote,
  Settings,
  Share2,
  SlidersHorizontal,
  ThumbsDown,
  ThumbsUp,
  Upload
} from "lucide-react";
import {
  createChatFlowSession,
  listChatFlowMessages,
  listChatFlowSessions,
  sendChatFlowMessage
} from "@/api/chat-flow";
import {
  createDataset,
  getParseJob,
  listDatasets,
  submitRagFeedback,
  uploadKnowledgeFile
} from "@/api/knowledge";
import type { ChatFlowMessageResp, ChatFlowSessionResp } from "@/types/chat-flow";
import type { CitationResp, DatasetResp, ParserJobResp, RagFeedbackRating } from "@/types/knowledge";

const notebookColors = ["bg-emerald-50", "bg-indigo-50", "bg-stone-100", "bg-rose-50", "bg-cyan-50"];
const notebookIcons = ["🔬", "🍂", "🐫", "⚙️", "🌊", "📁", "⚠️"];
const studioItems = [
  ["音频概览", "bg-indigo-50 text-indigo-800"],
  ["演示文稿", "bg-stone-100 text-stone-800"],
  ["视频概览", "bg-emerald-50 text-emerald-800"],
  ["思维导图", "bg-fuchsia-50 text-fuchsia-800"],
  ["报告", "bg-yellow-50 text-yellow-800"],
  ["闪卡", "bg-rose-50 text-rose-800"],
  ["测验", "bg-cyan-50 text-cyan-800"],
  ["数据表格", "bg-blue-50 text-blue-800"]
];

export function ChatAppClient() {
  const [datasets, setDatasets] = useState<DatasetResp[]>([]);
  const [sessions, setSessions] = useState<ChatFlowSessionResp[]>([]);
  const [activeDatasetId, setActiveDatasetId] = useState<number | null>(null);
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [messages, setMessages] = useState<ChatFlowMessageResp[]>([]);
  const [notebookOpen, setNotebookOpen] = useState(false);
  const [input, setInput] = useState("Which source should I trust?");
  const [parseJobs, setParseJobs] = useState<Record<number, ParserJobResp>>({});
  const [creating, setCreating] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [sending, setSending] = useState(false);
  const [feedbackSubmitting, setFeedbackSubmitting] = useState(false);
  const [feedbackStatus, setFeedbackStatus] = useState("");

  const refreshDatasets = useCallback(async () => {
    const page = await listDatasets({ page: 1, size: 50 });
    setDatasets(page.list);
    return page.list;
  }, []);

  const refreshSessions = useCallback(async () => {
    const nextSessions = await listChatFlowSessions({ mode: "knowledge" });
    setSessions(nextSessions);
    return nextSessions;
  }, []);

  useEffect(() => {
    void refreshDatasets().catch(() => setDatasets([]));
    void refreshSessions().catch(() => setSessions([]));
  }, [refreshDatasets, refreshSessions]);

  const activeDataset = useMemo(
    () => datasets.find((dataset) => dataset.id === activeDatasetId) ?? null,
    [datasets, activeDatasetId]
  );

  const activeSession = useMemo(
    () => sessions.find((session) => session.id === activeSessionId) ?? null,
    [sessions, activeSessionId]
  );

  const latestAssistant = useMemo(
    () => [...messages].reverse().find((message) => message.role === "assistant") ?? null,
    [messages]
  );

  async function handleCreateNotebook() {
    if (creating) {
      return;
    }
    setCreating(true);
    try {
      const datasetId = await createDataset({
        name: "未命名的笔记本",
        description: "Created from chat workspace",
        visibility: 1,
        retrievalMode: 3
      });
      const nextDatasets = await refreshDatasets();
      const created = nextDatasets.find((dataset) => dataset.id === datasetId);
      if (created) {
        openNotebook(created, sessions);
      }
    } finally {
      setCreating(false);
    }
  }

  async function openNotebook(dataset: DatasetResp, sessionSource = sessions) {
    setActiveDatasetId(dataset.id);
    setNotebookOpen(true);
    setFeedbackStatus("");
    let matchingSession = sessionSource.find(
      (session) => session.mode === "knowledge" && session.datasetId === dataset.id
    );
    if (!matchingSession) {
      const latestSessions = await refreshSessions().catch(() => []);
      matchingSession = latestSessions.find(
        (session) => session.mode === "knowledge" && session.datasetId === dataset.id
      );
    }
    setActiveSessionId(matchingSession?.id ?? null);
    if (matchingSession) {
      const history = await listChatFlowMessages(matchingSession.id).catch(() => []);
      setMessages(history);
    } else {
      setMessages([]);
    }
  }

  async function ensureKnowledgeSession(dataset: DatasetResp) {
    if (activeSessionId) {
      return activeSessionId;
    }
    const latestSessions = await refreshSessions().catch(() => sessions);
    const existing = latestSessions.find(
      (session) => session.mode === "knowledge" && session.datasetId === dataset.id
    );
    if (existing) {
      setActiveSessionId(existing.id);
      return existing.id;
    }
    const created = await createChatFlowSession({
      mode: "knowledge",
      datasetId: dataset.id,
      title: dataset.name
    });
    setSessions((current) => [created, ...current.filter((item) => item.id !== created.id)]);
    setActiveSessionId(created.id);
    return created.id;
  }

  async function handleSourceUpload(files: FileList | null) {
    if (!activeDataset || !files || files.length === 0 || uploading) {
      return;
    }
    const file = files[0];
    setUploading(true);
    try {
      const response = await uploadKnowledgeFile(activeDataset.id, file);
      setParseJobs((current) => ({ ...current, [response.parseJob.id]: response.parseJob }));
      const latest = await getParseJob(activeDataset.id, response.parseJob.id);
      setParseJobs((current) => ({ ...current, [latest.id]: latest }));
      await refreshDatasets().catch(() => undefined);
    } finally {
      setUploading(false);
    }
  }

  async function handleSend() {
    const content = input.trim();
    if (!activeDataset || !content || sending) {
      return;
    }
    setSending(true);
    setFeedbackStatus("");
    try {
      const sessionId = await ensureKnowledgeSession(activeDataset);
      const response = await sendChatFlowMessage(sessionId, { content, limit: 5 });
      setSessions((current) => [response.session, ...current.filter((item) => item.id !== response.session.id)]);
      setActiveSessionId(response.session.id);
      setMessages((current) => [...current, response.userMessage, response.assistantMessage]);
    } finally {
      setSending(false);
    }
  }

  async function handleFeedback(rating: RagFeedbackRating) {
    if (!latestAssistant?.ragTraceId || feedbackSubmitting) {
      return;
    }
    setFeedbackSubmitting(true);
    setFeedbackStatus("");
    try {
      await submitRagFeedback({
        traceId: latestAssistant.ragTraceId,
        rating,
        reason: "chat-answer-feedback"
      });
      setFeedbackStatus("反馈已保存");
    } finally {
      setFeedbackSubmitting(false);
    }
  }

  if (!notebookOpen || !activeDataset) {
    return (
      <main className="min-h-screen bg-white text-neutral-950">
        <TopBar title="NotebookLM" onCreateNotebook={handleCreateNotebook} />
        <section className="mx-auto max-w-[1640px] px-8 pb-12 pt-5">
          <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
            <button
              className="flex min-h-[230px] flex-col items-center justify-center rounded-lg border border-slate-200 bg-white p-6 text-center hover:border-slate-300"
              disabled={creating}
              onClick={handleCreateNotebook}
              type="button"
            >
              <span className="flex h-20 w-20 items-center justify-center rounded-full bg-indigo-50 text-4xl text-indigo-600">
                <Plus aria-hidden="true" className="h-7 w-7" />
              </span>
              <span className="mt-5 text-2xl font-medium">新建笔记本</span>
            </button>

            {datasets.map((dataset, index) => (
              <button
                aria-label={`打开 ${dataset.name}`}
                className={[
                  "min-h-[230px] rounded-lg p-7 text-left hover:brightness-[0.98]",
                  notebookColors[index % notebookColors.length]
                ].join(" ")}
                key={dataset.id}
                onClick={() => void openNotebook(dataset)}
                type="button"
              >
                <div className="flex items-start justify-between">
                  <span className="text-5xl">{notebookIcons[index % notebookIcons.length]}</span>
                  <MoreVertical aria-hidden="true" className="h-5 w-5 text-neutral-600" />
                </div>
                <div className="mt-10 line-clamp-3 text-3xl font-medium leading-tight tracking-normal">
                  {dataset.name}
                </div>
                <div className="mt-5 text-base text-neutral-600">
                  {formatChineseDate(dataset.createTime)} · {dataset.documentCount} 个来源
                </div>
              </button>
            ))}
          </div>
        </section>
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-[#eef0fb] text-neutral-950">
      <TopBar title={activeDataset.name} onCreateNotebook={handleCreateNotebook} />
      <div className="grid min-h-[calc(100vh-88px)] grid-cols-1 gap-4 px-5 pb-4 lg:grid-cols-[500px_minmax(0,1fr)_500px]">
        <aside className="flex min-h-[520px] flex-col overflow-hidden rounded-lg bg-white">
          <PanelHeader title="来源" />
          <div className="space-y-4 overflow-auto p-5">
            <label className="flex h-11 cursor-pointer items-center justify-center gap-2 rounded-full border border-slate-300 bg-white text-sm font-medium hover:bg-slate-50">
              <Plus aria-hidden="true" className="h-4 w-4" />
              添加来源
              <input
                accept=".txt,.md,.pdf,.csv,.json,.log,text/*,application/pdf,application/json"
                aria-label="添加来源"
                className="sr-only"
                disabled={uploading}
                onChange={(event) => void handleSourceUpload(event.target.files)}
                type="file"
              />
            </label>
            <SourceSearch />
            <SourceList dataset={activeDataset} parseJobs={Object.values(parseJobs)} />
          </div>
        </aside>

        <section className="flex min-h-[520px] min-w-0 flex-col overflow-hidden rounded-lg bg-white">
          <PanelHeader title="对话">
            <SlidersHorizontal aria-hidden="true" className="h-5 w-5 text-neutral-600" />
            <MoreVertical aria-hidden="true" className="h-5 w-5 text-neutral-600" />
          </PanelHeader>
          <div className="min-h-0 flex-1 overflow-auto px-6 py-5">
            <Conversation messages={messages} />
          </div>
          <div className="px-6 pb-5">
            <div className="mx-auto max-w-5xl rounded-2xl border border-neutral-400 bg-white px-5 py-4">
              <div className="flex items-center gap-3">
                <input
                  aria-label="提问或创作内容"
                  className="min-w-0 flex-1 bg-transparent text-base outline-none"
                  onChange={(event) => setInput(event.target.value)}
                  placeholder="提问或创作内容"
                  value={input}
                />
                <span className="hidden text-sm text-neutral-500 sm:inline">{activeDataset.documentCount} 个来源</span>
                <button
                  aria-label="发送消息"
                  className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-neutral-100 text-neutral-800 hover:bg-neutral-200 disabled:opacity-50"
                  disabled={sending}
                  onClick={() => void handleSend()}
                  type="button"
                >
                  <ArrowRight aria-hidden="true" className="h-6 w-6" />
                </button>
              </div>
            </div>
            <div className="mt-3 text-center text-xs text-neutral-500">
              NotebookLM 提供的内容未必准确，因此请仔细核查回答内容。
            </div>
          </div>
        </section>

        <aside className="flex min-h-[520px] flex-col overflow-hidden rounded-lg bg-white">
          <PanelHeader title="Studio" />
          <div className="min-h-0 flex-1 overflow-auto p-5">
            <div className="grid grid-cols-2 gap-3">
              {studioItems.map(([label, className]) => (
                <button
                  className={`flex min-h-20 items-center justify-between rounded-lg p-4 text-left text-sm font-semibold ${className}`}
                  key={label}
                  type="button"
                >
                  <span>{label}</span>
                  <ChevronRight aria-hidden="true" className="h-5 w-5" />
                </button>
              ))}
            </div>
            <div className="mt-6 border-t border-slate-200 pt-5">
              <StudioNotes dataset={activeDataset} activeSession={activeSession} />
            </div>
          </div>
          <div className="p-5">
            <button className="ml-auto flex h-11 items-center gap-2 rounded-full bg-black px-5 text-sm font-semibold text-white" type="button">
              <FileText aria-hidden="true" className="h-4 w-4" />
              添加笔记
            </button>
          </div>
        </aside>
      </div>
    </main>
  );

  function Conversation({ messages }: { messages: ChatFlowMessageResp[] }) {
    if (messages.length === 0) {
      return (
        <div className="mx-auto max-w-4xl pt-6 text-base leading-8 text-neutral-800">
          <p>
            选择左侧来源后，可以直接提问。回答会绑定当前笔记本、保存会话，并返回可追踪引用。
          </p>
        </div>
      );
    }

    return (
      <div className="mx-auto max-w-4xl space-y-6">
        {messages.map((message) =>
          message.role === "assistant" ? (
            <AssistantAnswer
              feedbackStatus={feedbackStatus}
              feedbackSubmitting={feedbackSubmitting}
              key={message.id}
              message={message}
              onFeedback={handleFeedback}
            />
          ) : (
            <div className="flex justify-end" key={message.id}>
              <div className="max-w-[78%] rounded-2xl bg-neutral-900 px-4 py-3 text-sm leading-6 text-white">
                {message.content}
              </div>
            </div>
          )
        )}
      </div>
    );
  }
}

function TopBar({ title, onCreateNotebook }: { title: string; onCreateNotebook: () => void }) {
  return (
    <header className="flex h-[88px] items-center justify-between px-6">
      <div className="flex min-w-0 items-center gap-4">
        <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-black text-white">
          <LayoutGrid aria-hidden="true" className="h-6 w-6" />
        </div>
        <h1 className="truncate text-3xl font-medium tracking-normal">{title}</h1>
      </div>
      <div className="flex shrink-0 items-center gap-3">
        <button
          className="hidden h-11 items-center gap-2 rounded-full bg-black px-5 text-sm font-semibold text-white md:flex"
          onClick={onCreateNotebook}
          type="button"
        >
          <Plus aria-hidden="true" className="h-4 w-4" />
          创建笔记本
        </button>
        <button className="hidden h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium md:flex" type="button">
          <Share2 aria-hidden="true" className="h-4 w-4" />
          分享
        </button>
        <button className="flex h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium" type="button">
          <Settings aria-hidden="true" className="h-4 w-4" />
          设置
        </button>
      </div>
    </header>
  );
}

function PanelHeader({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="flex h-16 items-center justify-between border-b border-slate-200 px-5">
      <h2 className="text-xl font-medium">{title}</h2>
      <div className="flex items-center gap-4">{children ?? <PanelLeft aria-hidden="true" className="h-5 w-5 text-neutral-600" />}</div>
    </div>
  );
}

function SourceSearch() {
  return (
    <div className="rounded-2xl border border-slate-200 p-3">
      <div className="text-base text-neutral-500">在网络中搜索新来源</div>
      <div className="mt-3 flex items-center gap-2">
        <span className="rounded-full border border-slate-200 px-3 py-2 text-sm font-semibold">Web</span>
        <span className="rounded-full border border-slate-200 px-3 py-2 text-sm font-semibold">Fast Research</span>
        <button className="ml-auto flex h-10 w-10 items-center justify-center rounded-full bg-neutral-100" type="button">
          <Upload aria-hidden="true" className="h-4 w-4 text-neutral-600" />
        </button>
      </div>
    </div>
  );
}

function SourceList({ dataset, parseJobs }: { dataset: DatasetResp; parseJobs: ParserJobResp[] }) {
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between text-sm text-neutral-700">
        <span className="font-medium">已选择 {dataset.documentCount} 个来源</span>
        <span>全选</span>
      </div>
      {parseJobs.map((job) => (
        <div className="flex items-center gap-3 rounded-lg px-2 py-2" key={job.id}>
          <FileText aria-hidden="true" className="h-5 w-5 shrink-0 text-red-500" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium">{job.documentName}</div>
            <div className="mt-1 text-xs text-neutral-500">{parseJobStatus(job)}</div>
          </div>
          <Check aria-hidden="true" className="h-5 w-5 text-neutral-500" />
        </div>
      ))}
      {parseJobs.length === 0 ? (
        <div className="rounded-lg border border-dashed border-slate-200 p-4 text-sm leading-6 text-neutral-500">
          还没有上传来源。添加文件后会进入 RAG 解析和索引。
        </div>
      ) : null}
    </div>
  );
}

function AssistantAnswer({
  message,
  feedbackStatus,
  feedbackSubmitting,
  onFeedback
}: {
  message: ChatFlowMessageResp;
  feedbackStatus: string;
  feedbackSubmitting: boolean;
  onFeedback: (rating: RagFeedbackRating) => void;
}) {
  const retrievalHitCount = Number(message.metadata.retrievalHitCount ?? 0);
  const answerStrategy = String(message.metadata.answerStrategy ?? "rag");

  return (
    <article className="text-base leading-8 text-neutral-800">
      <div className="flex items-start gap-3">
        <Bot aria-hidden="true" className="mt-1 h-5 w-5 shrink-0 text-neutral-600" />
        <div className="min-w-0 flex-1">
          <div>{message.content}</div>
          <div className="mt-3 flex flex-wrap gap-2 text-sm text-neutral-500">
            <span>Trace #{message.ragTraceId ?? 0}</span>
            <span>·</span>
            <span>{retrievalHitCount} hits</span>
            <span>·</span>
            <span>{answerStrategy}</span>
          </div>
          <CitationList citations={message.citations} />
          <div className="mt-4 flex flex-wrap items-center gap-3">
            <button
              className="rounded-full border border-slate-200 px-3 py-1.5 text-sm"
              disabled={!message.ragTraceId || feedbackSubmitting}
              type="button"
            >
              保存到笔记
            </button>
            <button
              aria-label="有帮助"
              className="text-neutral-600 disabled:opacity-40"
              disabled={!message.ragTraceId || feedbackSubmitting}
              onClick={() => onFeedback("helpful")}
              type="button"
            >
              <ThumbsUp aria-hidden="true" className="h-5 w-5" />
            </button>
            <button
              aria-label="答案不准确"
              className="text-neutral-600 disabled:opacity-40"
              disabled={!message.ragTraceId || feedbackSubmitting}
              onClick={() => onFeedback("not_helpful")}
              type="button"
            >
              <ThumbsDown aria-hidden="true" className="h-5 w-5" />
            </button>
            <button
              aria-label="引用问题"
              className="text-neutral-600 disabled:opacity-40"
              disabled={!message.ragTraceId || feedbackSubmitting}
              onClick={() => onFeedback("citation_issue")}
              type="button"
            >
              <Quote aria-hidden="true" className="h-5 w-5" />
            </button>
            {feedbackStatus ? <span className="text-sm text-neutral-500">{feedbackStatus}</span> : null}
          </div>
        </div>
      </div>
    </article>
  );
}

function CitationList({ citations }: { citations: CitationResp[] }) {
  if (citations.length === 0) {
    return null;
  }
  return (
    <div className="mt-3 flex flex-wrap gap-2">
      {citations.map((citation) => (
        <span className="rounded-full bg-neutral-100 px-3 py-1 text-sm text-neutral-600" key={citation.chunkId}>
          {citation.chunkId}
          {citation.pageNo ? ` · page ${citation.pageNo}` : ""}
        </span>
      ))}
    </div>
  );
}

function StudioNotes({
  dataset,
  activeSession
}: {
  dataset: DatasetResp;
  activeSession: ChatFlowSessionResp | null;
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-start gap-3">
        <Database aria-hidden="true" className="mt-1 h-6 w-6 text-blue-700" />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold">{dataset.name}</div>
          <div className="mt-1 text-sm text-neutral-500">{dataset.documentCount} 个来源 · 当前笔记本</div>
        </div>
        <MoreVertical aria-hidden="true" className="h-5 w-5 text-neutral-500" />
      </div>
      <div className="flex items-start gap-3">
        <FileText aria-hidden="true" className="mt-1 h-6 w-6 text-blue-700" />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold">{activeSession?.title ?? "未命名的笔记"}</div>
          <div className="mt-1 text-sm text-neutral-500">{activeSession?.messageCount ?? 0} 条消息</div>
        </div>
        <MoreVertical aria-hidden="true" className="h-5 w-5 text-neutral-500" />
      </div>
    </div>
  );
}

function parseJobStatus(job: ParserJobResp) {
  if (job.status === 3 || job.ingestionStatus === 4) {
    return `解析完成 · ${job.chunkCount} chunks`;
  }
  if (job.status === 4 || job.parseStatus === 4) {
    return "解析失败";
  }
  return "解析中";
}

function formatChineseDate(value: string) {
  const [date] = value.split(" ");
  const [year, month, day] = date.split("-");
  return `${Number(year)}年${Number(month)}月${Number(day)}日`;
}
