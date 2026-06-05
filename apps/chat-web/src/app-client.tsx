"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Bot,
  Database,
  FileText,
  History,
  MessageSquare,
  Paperclip,
  Quote,
  Search,
  Send,
  ShieldCheck,
  Sparkles,
  ThumbsDown,
  ThumbsUp,
  X
} from "lucide-react";
import { askDataset, listDatasets, submitRagFeedback } from "@/api/knowledge";
import { chatCompletion, listChatConversations } from "@/api/model";
import type { CitationResp, DatasetResp, RagAskResp, RagFeedbackRating } from "@/types/knowledge";
import type { ModelChatConversationResp, ModelChatFileContext, ModelChatResp } from "@/types/model";

const fallbackDataset: DatasetResp = {
  id: 10,
  tenantId: 1,
  name: "企业制度知识库",
  description: "政策、FAQ、流程",
  ownerId: 1,
  visibility: 1,
  status: 1,
  retrievalMode: 3,
  documentCount: 0,
  chunkCount: 0,
  createUserString: "admin",
  createTime: "2026-06-05 10:00:00",
  updateUserString: "",
  updateTime: ""
};

const fallbackAnswer: RagAskResp = {
  traceId: 0,
  answer: "Select a knowledge source and ask a question. Answers will include citations when live RAG is available.",
  citations: [],
  retrievalHitCount: 0,
  answerStrategy: "standby"
};

const fallbackModelAnswer: ModelChatResp = {
  conversationId: null,
  answer: "Start a model chat to use the configured LLM route without knowledge retrieval.",
  routeId: "runtime.llm",
  model: "standby",
  latencyMs: 0,
  usage: {
    promptTokens: null,
    completionTokens: null,
    totalTokens: null
  }
};

const modelRoutes = [
  { label: "Embedding", value: "local-keyword" },
  { label: "Rerank", value: "none" },
  { label: "Answer", value: "local-extractive" }
];

type ChatMode = "knowledge" | "model";

export function ChatAppClient() {
  const [mode, setMode] = useState<ChatMode>("knowledge");
  const [datasets, setDatasets] = useState<DatasetResp[]>([fallbackDataset]);
  const [selectedDatasetId, setSelectedDatasetId] = useState(fallbackDataset.id);
  const [question, setQuestion] = useState("Which handbook should I use?");
  const [answer, setAnswer] = useState<RagAskResp>(fallbackAnswer);
  const [modelQuestion, setModelQuestion] = useState("Explain Novex.");
  const [modelAnswer, setModelAnswer] = useState<ModelChatResp>(fallbackModelAnswer);
  const [modelConversationId, setModelConversationId] = useState<number | null>(null);
  const [modelConversations, setModelConversations] = useState<ModelChatConversationResp[]>([]);
  const [modelFileContexts, setModelFileContexts] = useState<ModelChatFileContext[]>([]);
  const [apiStatus, setApiStatus] = useState("fallback");
  const [asking, setAsking] = useState(false);
  const [modelAsking, setModelAsking] = useState(false);
  const [feedbackSubmitting, setFeedbackSubmitting] = useState(false);
  const [feedbackStatus, setFeedbackStatus] = useState("");

  const refreshModelConversations = useCallback(async () => {
    try {
      const conversations = await listChatConversations();
      setModelConversations(conversations);
    } catch {
      setModelConversations([]);
    }
  }, []);

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

    refreshModelConversations();

    return () => {
      mounted = false;
    };
  }, [refreshModelConversations]);

  const selectedDataset = useMemo(
    () => datasets.find((dataset) => dataset.id === selectedDatasetId) ?? fallbackDataset,
    [datasets, selectedDatasetId]
  );

  async function handleAsk() {
    const trimmed = question.trim();
    if (!trimmed || asking) {
      return;
    }

    setAsking(true);
    setFeedbackStatus("");
    try {
      const response = await askDataset(selectedDataset.id, {
        question: trimmed,
        limit: 5
      });
      setAnswer(response);
      setApiStatus("live");
    } catch {
      setApiStatus("fallback");
    } finally {
      setAsking(false);
    }
  }

  async function handleModelChat() {
    const trimmed = modelQuestion.trim();
    if (!trimmed || modelAsking) {
      return;
    }

    setModelAsking(true);
    try {
      const response = await chatCompletion({
        conversationId: modelConversationId ?? undefined,
        messages: [{ role: "user", content: trimmed }],
        fileContexts: modelFileContexts.length > 0 ? modelFileContexts : undefined,
        temperature: 0.2,
        maxTokens: 1024
      });
      setModelAnswer(response);
      setModelConversationId(response.conversationId ?? modelConversationId);
      setApiStatus("live");
      await refreshModelConversations();
    } catch {
      setApiStatus("fallback");
    } finally {
      setModelAsking(false);
    }
  }

  async function handleModelFileChange(files: FileList | null) {
    if (!files || files.length === 0) {
      return;
    }

    const selected = Array.from(files).slice(0, 3);
    const contexts = await Promise.all(
      selected.map(async (file) => ({
        name: file.name,
        contentType: file.type || "text/plain",
        content: await file.text()
      }))
    );
    setModelFileContexts(contexts);
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
        reason: "chat-answer-feedback"
      });
      setFeedbackStatus("反馈已保存");
    } catch {
      setFeedbackStatus("反馈保存失败");
    } finally {
      setFeedbackSubmitting(false);
    }
  }

  return (
    <main className="min-h-screen bg-slate-100 text-slate-950">
      <div className="mx-auto grid min-h-screen max-w-[1440px] grid-cols-1 lg:grid-cols-[260px_minmax(0,1fr)_340px]">
        <aside className="border-b border-slate-200 bg-white p-4 lg:border-b-0 lg:border-r lg:p-5">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-teal-700 text-sm font-semibold text-white">
              KB
            </div>
            <div className="min-w-0">
              <div className="text-sm font-semibold text-slate-950">Novex</div>
              <div className="truncate text-xs text-slate-500">Knowledge Chat</div>
            </div>
          </div>

          <nav aria-label="Knowledge navigation" className="mt-5 space-y-2">
            {[
              { label: "知识库问答", icon: Search, mode: "knowledge" as const },
              { label: "模型对话", icon: MessageSquare, mode: "model" as const }
            ].map((item) => (
              <button
                className={[
                  "flex w-full items-center gap-3 rounded-lg border px-3 py-2 text-left text-sm font-medium",
                  item.mode === mode
                    ? "border-teal-200 bg-teal-50 text-teal-950"
                    : "border-transparent text-slate-600 hover:border-slate-200 hover:bg-slate-50"
                ].join(" ")}
                key={item.label}
                onClick={() => setMode(item.mode)}
                type="button"
              >
                <item.icon aria-hidden="true" className="h-4 w-4 shrink-0" />
                {item.label}
              </button>
            ))}
          </nav>

          <section className="mt-5 rounded-lg border border-slate-200 p-3">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
              <ShieldCheck aria-hidden="true" className="h-4 w-4 text-teal-700" />
              Access
            </div>
            <div className="mt-2 text-xs leading-5 text-slate-500">
              RBAC token, tenant filter, source visibility and trace audit.
            </div>
          </section>
        </aside>

        <section className="min-w-0 p-4 lg:p-6">
          <header className="flex flex-col gap-3 border-b border-slate-200 pb-4 md:flex-row md:items-start md:justify-between">
            <div className="min-w-0">
              <div className="text-sm font-medium text-teal-700">
                {mode === "knowledge" ? "Knowledge Q&A" : "Model Chat"}
              </div>
              <h1 className="mt-2 text-2xl font-semibold tracking-normal text-slate-950">
                {mode === "knowledge" ? "Novex Knowledge" : "Novex Chat"}
              </h1>
              <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-600">
                {mode === "knowledge"
                  ? "Ask across governed knowledge datasets and keep answers grounded with citations."
                  : "Chat with the configured LLM route before adding retrieval, tools, or workflows."}
              </p>
            </div>
            <span className="inline-flex w-fit items-center gap-2 rounded-md bg-white px-3 py-2 text-sm font-medium text-slate-700 ring-1 ring-slate-200">
              <Bot aria-hidden="true" className="h-4 w-4 text-teal-700" />
              {apiStatus === "live" ? (mode === "knowledge" ? "Live RAG" : "Live Model") : "Fallback"}
            </span>
          </header>

          <div className="mt-5 rounded-lg border border-slate-200 bg-white shadow-sm">
            <div className="border-b border-slate-200 p-4">
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div className="min-w-0">
                  <div className="text-sm font-semibold text-slate-950">Conversation</div>
                  <div className="mt-1 truncate text-xs text-slate-500">
                    {mode === "knowledge"
                      ? `Source: ${selectedDataset.name}`
                      : `${modelAnswer.routeId} · ${modelAnswer.model ?? "default"}`}
                  </div>
                </div>
                {mode === "knowledge" ? (
                  <select
                    aria-label="选择知识库"
                    className="h-9 rounded-md border border-slate-200 bg-white px-2 text-sm text-slate-700"
                    onChange={(event) => setSelectedDatasetId(Number(event.target.value))}
                    value={selectedDatasetId}
                  >
                    {datasets.map((dataset) => (
                      <option key={dataset.id} value={dataset.id}>
                        {dataset.name}
                      </option>
                    ))}
                  </select>
                ) : (
                  <span className="inline-flex h-9 items-center rounded-md border border-slate-200 px-3 text-sm text-slate-700">
                    {modelAnswer.latencyMs} ms
                  </span>
                )}
              </div>
            </div>

            <div className="min-h-[360px] space-y-4 p-4">
              <ChatBubble text={mode === "knowledge" ? question || "Ask a question" : modelQuestion || "Ask a model"} />
              <div className="rounded-lg border border-slate-200 bg-slate-50 p-4">
                <div className="flex items-start gap-3">
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-teal-700 text-white">
                    <Bot aria-hidden="true" className="h-4 w-4" />
                  </div>
                  <div className="min-w-0 flex-1">
                    {mode === "knowledge" ? (
                      <KnowledgeAnswer
                        answer={answer}
                        feedbackStatus={feedbackStatus}
                        feedbackSubmitting={feedbackSubmitting}
                        onFeedback={handleFeedback}
                      />
                    ) : (
                      <ModelAnswer answer={modelAnswer} />
                    )}
                  </div>
                </div>
              </div>
            </div>

            <div className="border-t border-slate-200 p-4">
              {mode === "model" && modelFileContexts.length > 0 ? (
                <div className="mb-3 flex flex-wrap gap-2">
                  {modelFileContexts.map((file) => (
                    <span
                      className="inline-flex max-w-full items-center gap-2 rounded-md border border-slate-200 bg-slate-50 px-2 py-1 text-xs font-medium text-slate-700"
                      key={file.name}
                    >
                      <FileText aria-hidden="true" className="h-3.5 w-3.5 shrink-0 text-teal-700" />
                      <span className="truncate">{file.name}</span>
                      <button
                        aria-label={`移除 ${file.name}`}
                        className="inline-flex h-5 w-5 items-center justify-center rounded text-slate-500 hover:bg-slate-200"
                        onClick={() => setModelFileContexts((current) => current.filter((item) => item.name !== file.name))}
                        type="button"
                      >
                        <X aria-hidden="true" className="h-3.5 w-3.5" />
                      </button>
                    </span>
                  ))}
                </div>
              ) : null}
              <div className="flex gap-2">
                {mode === "model" ? (
                  <label className="inline-flex h-10 w-10 shrink-0 cursor-pointer items-center justify-center rounded-lg border border-slate-200 bg-white text-slate-600 hover:bg-slate-50">
                    <Paperclip aria-hidden="true" className="h-4 w-4" />
                    <input
                      accept=".txt,.md,.csv,.json,.log,text/*,application/json"
                      aria-label="添加模型上下文文件"
                      className="sr-only"
                      multiple
                      onChange={(event) => void handleModelFileChange(event.target.files)}
                      type="file"
                    />
                  </label>
                ) : null}
                <input
                  aria-label={mode === "knowledge" ? "输入知识库问题" : "输入模型问题"}
                  className="min-w-0 flex-1 rounded-lg border border-slate-200 px-3 py-2 text-sm outline-none focus:border-teal-500"
                  onChange={(event) =>
                    mode === "knowledge" ? setQuestion(event.target.value) : setModelQuestion(event.target.value)
                  }
                  value={mode === "knowledge" ? question : modelQuestion}
                />
                <button
                  aria-label={mode === "knowledge" ? "发送问题" : "发送模型消息"}
                  className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-teal-700 text-white hover:bg-teal-800 disabled:bg-slate-300"
                  disabled={mode === "knowledge" ? asking : modelAsking}
                  onClick={mode === "knowledge" ? handleAsk : handleModelChat}
                  type="button"
                >
                  <Send aria-hidden="true" className="h-4 w-4" />
                </button>
              </div>
            </div>
          </div>
        </section>

        <aside className="space-y-4 border-t border-slate-200 bg-white p-4 lg:border-l lg:border-t-0 lg:p-5">
          {mode === "knowledge" ? (
            <KnowledgeSources datasets={datasets} selectedDatasetId={selectedDatasetId} onSelect={setSelectedDatasetId} />
          ) : (
            <ModelConversations
              activeId={modelConversationId}
              conversations={modelConversations}
              onSelect={(conversation) => {
                setModelConversationId(conversation.id);
                setModelQuestion(conversation.title);
              }}
            />
          )}

          <section className="rounded-lg border border-slate-200 p-4">
            <div className="flex items-center gap-2">
              <Sparkles aria-hidden="true" className="h-4 w-4 text-teal-700" />
              <h2 className="text-sm font-semibold text-slate-950">Trace</h2>
            </div>
            {mode === "knowledge" ? <KnowledgeTrace /> : <ModelTrace answer={modelAnswer} />}
          </section>
        </aside>
      </div>
    </main>
  );
}

function KnowledgeSources({
  datasets,
  selectedDatasetId,
  onSelect
}: {
  datasets: DatasetResp[];
  selectedDatasetId: number;
  onSelect: (datasetId: number) => void;
}) {
  return (
    <section className="rounded-lg border border-slate-200 p-4">
      <div className="flex items-center gap-2">
        <Database aria-hidden="true" className="h-4 w-4 text-teal-700" />
        <h2 className="text-sm font-semibold text-slate-950">Sources</h2>
      </div>
      <div className="mt-3 space-y-2">
        {datasets.map((dataset) => (
          <button
            className={[
              "w-full rounded-lg border p-3 text-left",
              dataset.id === selectedDatasetId ? "border-teal-200 bg-teal-50" : "border-slate-200 hover:bg-slate-50"
            ].join(" ")}
            key={dataset.id}
            onClick={() => onSelect(dataset.id)}
            type="button"
          >
            <div className="truncate text-sm font-semibold text-slate-900">{dataset.name}</div>
            <div className="mt-1 text-xs text-slate-500">
              {dataset.documentCount} docs · {dataset.chunkCount} chunks
            </div>
          </button>
        ))}
      </div>
    </section>
  );
}

function ModelConversations({
  activeId,
  conversations,
  onSelect
}: {
  activeId: number | null;
  conversations: ModelChatConversationResp[];
  onSelect: (conversation: ModelChatConversationResp) => void;
}) {
  return (
    <section className="rounded-lg border border-slate-200 p-4">
      <div className="flex items-center gap-2">
        <History aria-hidden="true" className="h-4 w-4 text-teal-700" />
        <h2 className="text-sm font-semibold text-slate-950">Recent Chats</h2>
      </div>
      <div className="mt-3 space-y-2">
        {conversations.length === 0 ? (
          <div className="rounded-lg border border-dashed border-slate-200 p-3 text-xs leading-5 text-slate-500">
            No model conversations yet.
          </div>
        ) : (
          conversations.map((conversation) => (
            <button
              className={[
                "w-full rounded-lg border p-3 text-left",
                conversation.id === activeId ? "border-teal-200 bg-teal-50" : "border-slate-200 hover:bg-slate-50"
              ].join(" ")}
              key={conversation.id}
              onClick={() => onSelect(conversation)}
              type="button"
            >
              <div className="truncate text-sm font-semibold text-slate-900">{conversation.title}</div>
              <div className="mt-1 line-clamp-2 text-xs leading-5 text-slate-500">{conversation.lastMessagePreview}</div>
              <div className="mt-2 text-xs text-slate-400">
                {conversation.messageCount} messages · {conversation.model ?? "default"}
              </div>
            </button>
          ))
        )}
      </div>
    </section>
  );
}

function ChatBubble({ text }: { text: string }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[80%] rounded-lg bg-slate-900 px-4 py-3 text-sm leading-6 text-white">
        {text}
      </div>
    </div>
  );
}

function KnowledgeAnswer({
  answer,
  feedbackStatus,
  feedbackSubmitting,
  onFeedback
}: {
  answer: RagAskResp;
  feedbackStatus: string;
  feedbackSubmitting: boolean;
  onFeedback: (rating: RagFeedbackRating) => void;
}) {
  return (
    <>
      <div className="text-sm leading-6 text-slate-800">{answer.answer}</div>
      <div className="mt-3 flex flex-wrap items-center gap-1 text-xs text-slate-500">
        <span>Trace #{answer.traceId}</span>
        <span>·</span>
        <span>{answer.retrievalHitCount} hits</span>
        <span>·</span>
        <span>{answer.answerStrategy}</span>
      </div>
      <CitationList citations={answer.citations} />
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <button
          aria-label="有帮助"
          className="inline-flex h-8 items-center gap-1 rounded-md border border-slate-200 bg-white px-2 text-xs font-medium text-slate-600 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={answer.traceId <= 0 || feedbackSubmitting}
          onClick={() => onFeedback("helpful")}
          type="button"
        >
          <ThumbsUp aria-hidden="true" className="h-3.5 w-3.5" />
          Helpful
        </button>
        <button
          aria-label="答案不准确"
          className="inline-flex h-8 items-center gap-1 rounded-md border border-slate-200 bg-white px-2 text-xs font-medium text-slate-600 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={answer.traceId <= 0 || feedbackSubmitting}
          onClick={() => onFeedback("not_helpful")}
          type="button"
        >
          <ThumbsDown aria-hidden="true" className="h-3.5 w-3.5" />
          Not accurate
        </button>
        <button
          aria-label="引用问题"
          className="inline-flex h-8 items-center gap-1 rounded-md border border-slate-200 bg-white px-2 text-xs font-medium text-slate-600 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={answer.traceId <= 0 || feedbackSubmitting}
          onClick={() => onFeedback("citation_issue")}
          type="button"
        >
          <Quote aria-hidden="true" className="h-3.5 w-3.5" />
          Citation issue
        </button>
        {feedbackStatus ? (
          <span className="inline-flex h-8 items-center rounded-md bg-slate-100 px-2 text-xs font-medium text-slate-600">
            {feedbackStatus}
          </span>
        ) : null}
      </div>
    </>
  );
}

function ModelAnswer({ answer }: { answer: ModelChatResp }) {
  return (
    <>
      <div className="text-sm leading-6 text-slate-800">{answer.answer}</div>
      <div className="mt-3 flex flex-wrap items-center gap-1 text-xs text-slate-500">
        <span>
          {answer.routeId} · {answer.model ?? "default"}
        </span>
        <span>·</span>
        <span>{answer.latencyMs} ms</span>
      </div>
    </>
  );
}

function KnowledgeTrace() {
  return (
    <div className="mt-3 space-y-2">
      {modelRoutes.map((route) => (
        <div className="rounded-lg bg-slate-50 p-3" key={route.label}>
          <div className="text-xs font-medium text-slate-500">{route.label}</div>
          <div className="mt-1 text-sm font-semibold text-slate-900">
            {route.label} {route.value}
          </div>
        </div>
      ))}
    </div>
  );
}

function ModelTrace({ answer }: { answer: ModelChatResp }) {
  return (
    <div className="mt-3 space-y-2">
      <div className="rounded-lg bg-slate-50 p-3">
        <div className="text-xs font-medium text-slate-500">Route</div>
        <div className="mt-1 text-sm font-semibold text-slate-900">
          {answer.routeId} · {answer.model ?? "default"}
        </div>
      </div>
      <div className="rounded-lg bg-slate-50 p-3">
        <div className="text-xs font-medium text-slate-500">Latency</div>
        <div className="mt-1 text-sm font-semibold text-slate-900">{answer.latencyMs} ms</div>
      </div>
      <div className="rounded-lg bg-slate-50 p-3">
        <div className="text-xs font-medium text-slate-500">Tokens</div>
        <div className="mt-1 text-sm font-semibold text-slate-900">
          {answer.usage.totalTokens ?? 0} total
        </div>
      </div>
    </div>
  );
}

function CitationList({ citations }: { citations: CitationResp[] }) {
  if (citations.length === 0) {
    return null;
  }

  return (
    <div className="mt-3 grid gap-2 md:grid-cols-2">
      {citations.map((citation) => (
        <article className="rounded-lg border border-slate-200 bg-white p-3" key={citation.chunkId}>
          <div className="flex items-center gap-2">
            <FileText aria-hidden="true" className="h-4 w-4 text-teal-700" />
            <div className="min-w-0 flex-1 truncate text-sm font-semibold text-slate-900">
              {citation.sectionPath[0] || `Document ${citation.documentId}`}
            </div>
          </div>
          <div className="mt-2 text-xs text-slate-500">
            {citation.chunkId}
            {citation.pageNo ? ` · page ${citation.pageNo}` : ""}
          </div>
        </article>
      ))}
    </div>
  );
}
