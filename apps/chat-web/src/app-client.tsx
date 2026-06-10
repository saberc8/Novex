"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type FormEvent, type KeyboardEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Transformer } from "markmap-lib";
import { Markmap } from "markmap-view";
import {
  AlertTriangle,
  ArrowRight,
  Bot,
  Check,
  ChevronRight,
  Copy,
  Database,
  FileText,
  GitBranch,
  LayoutGrid,
  Loader2,
  LogOut,
  MoreVertical,
  PanelLeft,
  Plus,
  Share2,
  SlidersHorizontal,
  Upload,
  X
} from "lucide-react";
import { accountLogin, getImageCaptcha } from "@/api/auth";
import {
  createChatFlowSession,
  listChatFlowMessages,
  listChatFlowSessions,
  sendChatFlowMessage
} from "@/api/chat-flow";
import { listSkills } from "@/api/capability";
import {
  createDataset,
  deleteDataset,
  getParseJob,
  listDocuments,
  listDatasets,
  uploadKnowledgeFile
} from "@/api/knowledge";
import { getModelRuntimeConfig } from "@/api/model";
import {
  generateStudioArtifact,
  listDatasetStudioArtifacts,
  listStudioActions
} from "@/api/studio";
import { clearAuthToken, getAuthToken, setAuthToken as persistAuthToken } from "@/lib/auth";
import type { AppRouteKey } from "@/page-routes";
import type { ChatFlowMessageResp, ChatFlowMode, ChatFlowSessionResp } from "@/types/chat-flow";
import type { ImageCaptchaResp } from "@/types/auth";
import type { CapabilityItemResp } from "@/types/capability";
import type { CitationResp, DatasetResp, DocumentResp, ParserJobResp } from "@/types/knowledge";
import type { ModelRoutePurpose, ModelRuntimeRouteSummary } from "@/types/model";
import type { MindMapContent, StudioActionResp, StudioArtifactResp } from "@/types/studio";

const CHAT_CLIENT_ID = "novex-chat-web";
const DEFAULT_MIND_MAP_MAX_NODES = 72;
const MIN_MIND_MAP_MAX_NODES = 12;
const MAX_MIND_MAP_MAX_NODES = 96;
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

type ToastState = {
  kind: "error" | "success";
  message: string;
};

export function ChatAppClient({
  activeRoute,
  initialDatasetId = null,
  mode = "knowledge"
}: {
  activeRoute?: AppRouteKey;
  initialDatasetId?: number | null;
  mode?: ChatFlowMode;
} = {}) {
  const router = useRouter();
  const isModelMode = mode === "model";
  const currentRoute = activeRoute ?? (isModelMode ? "chat" : "knowledge");
  const routeDatasetId = normalizeDatasetId(initialDatasetId);
  const runtimePurpose: ModelRoutePurpose = isModelMode ? "chat" : "rag_answer";
  const [authToken, setAuthToken] = useState<string | null>(null);
  const [loginUsername, setLoginUsername] = useState("admin");
  const [loginPassword, setLoginPassword] = useState("admin123");
  const [loginCaptcha, setLoginCaptcha] = useState("");
  const [captcha, setCaptcha] = useState<ImageCaptchaResp | null>(null);
  const [loggingIn, setLoggingIn] = useState(false);
  const [loginStatus, setLoginStatus] = useState("");
  const [datasets, setDatasets] = useState<DatasetResp[]>([]);
  const [documents, setDocuments] = useState<DocumentResp[]>([]);
  const [sessions, setSessions] = useState<ChatFlowSessionResp[]>([]);
  const [activeDatasetId, setActiveDatasetId] = useState<number | null>(routeDatasetId);
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [messages, setMessages] = useState<ChatFlowMessageResp[]>([]);
  const [notebookOpen, setNotebookOpen] = useState(Boolean(routeDatasetId));
  const [input, setInput] = useState(() => (isModelMode ? "Draft a concise rollout note." : ""));
  const [parseJobs, setParseJobs] = useState<Record<number, ParserJobResp>>({});
  const [documentLoading, setDocumentLoading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [deletingDatasetId, setDeletingDatasetId] = useState<number | null>(null);
  const [deleteCandidate, setDeleteCandidate] = useState<DatasetResp | null>(null);
  const [uploading, setUploading] = useState(false);
  const [sending, setSending] = useState(false);
  const [copiedMessageId, setCopiedMessageId] = useState<number | null>(null);
  const [actionStatus, setActionStatus] = useState("");
  const [sendStatus, setSendStatus] = useState("");
  const [sendError, setSendError] = useState("");
  const [toast, setToast] = useState<ToastState | null>(null);
  const [llmRoutes, setLlmRoutes] = useState<ModelRuntimeRouteSummary[]>([]);
  const [selectedLlmRouteId, setSelectedLlmRouteId] = useState("");
  const [skills, setSkills] = useState<CapabilityItemResp[]>([]);
  const [selectedSkill, setSelectedSkill] = useState<CapabilityItemResp | null>(null);
  const [skillPickerOpen, setSkillPickerOpen] = useState(false);
  const [skillFilter, setSkillFilter] = useState("");
  const [activeSkillIndex, setActiveSkillIndex] = useState(0);
  const [studioActions, setStudioActions] = useState<StudioActionResp[]>([]);
  const [studioArtifacts, setStudioArtifacts] = useState<StudioArtifactResp[]>([]);
  const [studioLoading, setStudioLoading] = useState(false);
  const [generatingActionCode, setGeneratingActionCode] = useState<string | null>(null);
  const [studioError, setStudioError] = useState("");
  const [openStudioArtifactId, setOpenStudioArtifactId] = useState<number | null>(null);
  const [mindMapPromptOpen, setMindMapPromptOpen] = useState(false);
  const [mindMapPrompt, setMindMapPrompt] = useState("");
  const [mindMapMaxNodes, setMindMapMaxNodes] = useState(DEFAULT_MIND_MAP_MAX_NODES);

  const resetClientSession = useCallback(() => {
    clearAuthToken();
    setAuthToken(null);
    setDatasets([]);
    setDocuments([]);
    setSessions([]);
    setActiveDatasetId(null);
    setActiveSessionId(null);
    setMessages([]);
    setNotebookOpen(false);
    setParseJobs({});
    setActionStatus("");
    setCopiedMessageId(null);
    setSendError("");
    setSendStatus("");
    setSkills([]);
    setSelectedSkill(null);
    setSkillPickerOpen(false);
    setSkillFilter("");
    setStudioActions([]);
    setStudioArtifacts([]);
    setStudioLoading(false);
    setGeneratingActionCode(null);
    setStudioError("");
  }, []);

  const handleAuthRejected = useCallback(
    (error: unknown) => {
      if (!isAuthRejectedError(error)) {
        return false;
      }
      resetClientSession();
      setLoginStatus("登录已过期，请重新登录");
      return true;
    },
    [resetClientSession]
  );

  useEffect(() => {
    setAuthToken(getAuthToken());
  }, []);

  const refreshDatasets = useCallback(async () => {
    const page = await listDatasets({ page: 1, size: 50 });
    setDatasets(page.list);
    return page.list;
  }, []);

  const refreshSessions = useCallback(async () => {
    const nextSessions = await listChatFlowSessions({ mode });
    setSessions(nextSessions);
    return nextSessions;
  }, [mode]);

  const refreshDocuments = useCallback(async (datasetId: number) => {
    setDocumentLoading(true);
    try {
      const page = await listDocuments(datasetId, { page: 1, size: 100 });
      setDocuments(page.list);
      return page.list;
    } finally {
      setDocumentLoading(false);
    }
  }, []);

  const refreshModelRuntime = useCallback(async () => {
    const summary = await getModelRuntimeConfig();
    const routes = summary.routes.filter(
      (route) => route.target === "llm" && route.purposes.includes(runtimePurpose)
    );
    setLlmRoutes(routes);
    setSelectedLlmRouteId((current) =>
      current && routes.some((route) => route.routeId === current)
        ? current
        : routes[0]?.routeId ?? ""
    );
    return routes;
  }, [runtimePurpose]);

  const refreshSkills = useCallback(async () => {
    const page = await listSkills({ page: 1, size: 50, status: 1 });
    setSkills(page.list);
    return page.list;
  }, []);

  const refreshStudioActions = useCallback(async () => {
    const actions = await listStudioActions({ surface: "knowledge" });
    setStudioActions(actions);
    setStudioError("");
    return actions;
  }, []);

  const refreshStudioArtifacts = useCallback(async (datasetId: number) => {
    setStudioLoading(true);
    try {
      const artifacts = await listDatasetStudioArtifacts(datasetId);
      setStudioArtifacts(artifacts);
      return artifacts;
    } finally {
      setStudioLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!authToken) {
      return;
    }
    void refreshModelRuntime().catch((error) => {
      if (handleAuthRejected(error)) {
        return;
      }
      setLlmRoutes([]);
      setSelectedLlmRouteId("");
    });
    if (!isModelMode) {
      void refreshSkills().catch((error) => {
        if (handleAuthRejected(error)) {
          return;
        }
        setSkills([]);
      });
      void refreshStudioActions().catch((error) => {
        if (handleAuthRejected(error)) {
          return;
        }
        setStudioActions([]);
        setStudioError(
          `Studio 功能加载失败：${error instanceof Error ? error.message : "请稍后重试"}`
        );
      });
    }
    if (isModelMode) {
      void refreshSessions()
        .then(async (nextSessions) => {
          const existing = nextSessions.find((session) => session.mode === "model");
          setActiveSessionId(existing?.id ?? null);
          if (existing) {
            setMessages(await listChatFlowMessages(existing.id).catch(() => []));
          } else {
            setMessages([]);
          }
        })
        .catch((error) => {
          if (handleAuthRejected(error)) {
            return;
          }
          setSessions([]);
        });
      return;
    }
    void refreshDatasets().catch((error) => {
      if (handleAuthRejected(error)) {
        return;
      }
      setDatasets([]);
    });
    void refreshSessions().catch((error) => {
      if (handleAuthRejected(error)) {
        return;
      }
      setSessions([]);
    });
  }, [
    authToken,
    handleAuthRejected,
    isModelMode,
    refreshDatasets,
    refreshModelRuntime,
    refreshSessions,
    refreshStudioActions,
    refreshSkills
  ]);

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
  const selectedLlmRoute = useMemo(
    () => llmRoutes.find((route) => route.routeId === selectedLlmRouteId) ?? llmRoutes[0] ?? null,
    [llmRoutes, selectedLlmRouteId]
  );
  const selectedRuntimeRouteId = selectedLlmRoute
    ? purposeRouteId(selectedLlmRoute, runtimePurpose)
    : undefined;
  const filteredSkills = useMemo(
    () => filterSkills(skills, skillFilter),
    [skills, skillFilter]
  );
  const mindMapAction = useMemo(
    () => studioActions.find((action) => action.code === "mind_map.generate") ?? null,
    [studioActions]
  );
  const latestMindMapArtifact = useMemo(
    () => studioArtifacts.find((artifact) => artifact.artifactType === "mind_map") ?? null,
    [studioArtifacts]
  );
  const openStudioArtifact = useMemo(
    () => studioArtifacts.find((artifact) => artifact.id === openStudioArtifactId) ?? null,
    [openStudioArtifactId, studioArtifacts]
  );

  useEffect(() => {
    setActiveSkillIndex(0);
  }, [skillFilter, skills.length]);

  useEffect(() => {
    if (isModelMode) {
      return;
    }
    if (routeDatasetId) {
      setActiveDatasetId(routeDatasetId);
      setNotebookOpen(true);
      return;
    }
    setActiveDatasetId(null);
    setNotebookOpen(false);
    setDocuments([]);
    setStudioArtifacts([]);
    setMessages([]);
    setActiveSessionId(null);
  }, [isModelMode, routeDatasetId]);

  useEffect(() => {
    if (!authToken || isModelMode || !activeDatasetId) {
      return;
    }
    if (!activeDataset) {
      if (datasets.length > 0) {
        setActionStatus(`未找到知识库 #${activeDatasetId}`);
      }
      return;
    }

    let cancelled = false;
    setNotebookOpen(true);
    setActionStatus("");
    setCopiedMessageId(null);
    setSendError("");
    setSendStatus("");
    setDocuments([]);
    setStudioArtifacts([]);
    void refreshDocuments(activeDataset.id).catch((error) => {
      if (handleAuthRejected(error)) {
        return;
      }
      if (!cancelled) {
        setDocuments([]);
      }
    });
    void refreshStudioArtifacts(activeDataset.id).catch((error) => {
      if (handleAuthRejected(error)) {
        return;
      }
      if (!cancelled) {
        setStudioArtifacts([]);
      }
    });
    void refreshSessions()
      .then(async (nextSessions) => {
        const matchingSession = nextSessions.find(
          (session) => session.mode === "knowledge" && session.datasetId === activeDataset.id
        );
        const history = matchingSession
          ? await listChatFlowMessages(matchingSession.id).catch((error) => {
              handleAuthRejected(error);
              return [];
            })
          : [];
        if (cancelled) {
          return;
        }
        setActiveSessionId(matchingSession?.id ?? null);
        setMessages(history);
      })
      .catch((error) => {
        if (handleAuthRejected(error)) {
          return;
        }
        if (!cancelled) {
          setActiveSessionId(null);
          setMessages([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    activeDataset?.id,
    activeDatasetId,
    authToken,
    datasets.length,
    handleAuthRejected,
    isModelMode,
    refreshDocuments,
    refreshSessions,
    refreshStudioArtifacts
  ]);

  async function handleCreateNotebook() {
    if (creating) {
      return;
    }
    setCreating(true);
    setActionStatus("");
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
        openNotebook(created);
      }
    } catch (error) {
      setActionStatus(error instanceof Error ? error.message : "创建笔记本失败");
    } finally {
      setCreating(false);
    }
  }

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
        clientId: CHAT_CLIENT_ID,
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
    } catch (error) {
      setLoginStatus(error instanceof Error ? error.message : "登录失败");
    } finally {
      setLoggingIn(false);
    }
  }

  function handleLogout() {
    resetClientSession();
  }

  function openNotebook(dataset: DatasetResp) {
    setActiveDatasetId(dataset.id);
    setNotebookOpen(true);
    router.push(datasetDetailHref(dataset.id, currentRoute));
  }

  function showToast(message: string, kind: ToastState["kind"] = "error") {
    setToast({ kind, message });
  }

  function handleDeleteNotebook(dataset: DatasetResp) {
    if (deletingDatasetId !== null) {
      return;
    }
    setActionStatus("");
    setDeleteCandidate(dataset);
  }

  function handleCancelDeleteNotebook() {
    if (deletingDatasetId !== null) {
      return;
    }
    setDeleteCandidate(null);
  }

  async function handleConfirmDeleteNotebook() {
    const dataset = deleteCandidate;
    if (!dataset || deletingDatasetId !== null) {
      return;
    }

    setDeletingDatasetId(dataset.id);
    try {
      await deleteDataset(dataset.id);
      setDatasets((current) => current.filter((item) => item.id !== dataset.id));
      setSessions((current) => current.filter((session) => session.datasetId !== dataset.id));
      setParseJobs((current) =>
        Object.fromEntries(
          Object.entries(current).filter(([, job]) => job.datasetId !== dataset.id)
        )
      );
      if (activeDatasetId === dataset.id) {
        setActiveDatasetId(null);
        setActiveSessionId(null);
        setDocuments([]);
        setMessages([]);
        setNotebookOpen(false);
        router.push("/knowledge");
      }
      setDeleteCandidate(null);
      showToast(`已删除知识库「${dataset.name}」`, "success");
    } catch (error) {
      setDeleteCandidate(null);
      showToast(error instanceof Error ? error.message : "删除知识库失败");
    } finally {
      setDeletingDatasetId(null);
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

  async function ensureModelSession() {
    if (activeSessionId) {
      return activeSessionId;
    }
    const latestSessions = sessions.length > 0 ? sessions : await refreshSessions().catch(() => []);
    const existing = latestSessions.find((session) => session.mode === "model");
    if (existing) {
      setActiveSessionId(existing.id);
      return existing.id;
    }
    const created = await createChatFlowSession({
      mode: "model",
      title: "Model Chat"
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
      await refreshDocuments(activeDataset.id).catch(() => undefined);
    } finally {
      setUploading(false);
    }
  }

  function handleComposerChange(value: string) {
    setInput(value);
    if (isModelMode) {
      return;
    }
    const nextFilter = skillQueryFromInput(value);
    if (nextFilter === null) {
      setSkillPickerOpen(false);
      setSkillFilter("");
      return;
    }
    setSkillFilter(nextFilter);
    setSkillPickerOpen(true);
  }

  function handleComposerKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (isModelMode) {
      return;
    }
    if (event.key === "/" && input.trim().length === 0) {
      setSkillPickerOpen(true);
      setSkillFilter("");
      return;
    }
    if (!skillPickerOpen) {
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      setSkillPickerOpen(false);
      setSkillFilter("");
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveSkillIndex((current) => Math.min(current + 1, Math.max(filteredSkills.length - 1, 0)));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveSkillIndex((current) => Math.max(current - 1, 0));
      return;
    }
    if ((event.key === "Enter" || event.key === "Tab") && filteredSkills[activeSkillIndex]) {
      event.preventDefault();
      selectSkill(filteredSkills[activeSkillIndex]);
    }
  }

  function selectSkill(skill: CapabilityItemResp) {
    setSelectedSkill(skill);
    setInput((current) => removeSkillSlashTrigger(current));
    setSkillPickerOpen(false);
    setSkillFilter("");
  }

  async function handleSend() {
    const content = input.trim();
    if (!content || sending) {
      return;
    }
    setSending(true);
    setCopiedMessageId(null);
    setSendError("");
    setSendStatus(isModelMode ? "正在调用模型..." : "正在检索知识库并生成回答...");
    try {
      if (isModelMode) {
        const sessionId = await ensureModelSession();
        const response = await sendChatFlowMessage(sessionId, {
          content,
          modelRouteId: selectedRuntimeRouteId
        });
        setSessions((current) => [response.session, ...current.filter((item) => item.id !== response.session.id)]);
        setActiveSessionId(response.session.id);
        setMessages((current) => [...current, response.userMessage, response.assistantMessage]);
        setInput("");
        setSendStatus("");
        return;
      }
      if (!activeDataset) {
        setSendError("请先选择知识库");
        return;
      }
      const sessionId = await ensureKnowledgeSession(activeDataset);
      const response = await sendChatFlowMessage(sessionId, {
        content,
        limit: 5,
        answerModelRouteId: selectedRuntimeRouteId,
        ...(selectedSkill ? { skillCode: selectedSkill.code } : {})
      });
      setSessions((current) => [response.session, ...current.filter((item) => item.id !== response.session.id)]);
      setActiveSessionId(response.session.id);
      setMessages((current) => [...current, response.userMessage, response.assistantMessage]);
      setInput("");
      setSelectedSkill(null);
      setSkillPickerOpen(false);
      setSkillFilter("");
      setSendStatus("");
    } catch (error) {
      setSendStatus("");
      setSendError(error instanceof Error ? error.message : "发送失败");
    } finally {
      setSending(false);
    }
  }

  async function handleGenerateStudioArtifact(
    action: StudioActionResp,
    options: { topic?: string; maxNodes?: number } = {}
  ) {
    if (!activeDataset || generatingActionCode) {
      return false;
    }
    const topic = options.topic?.trim() || activeDataset.name;
    const maxNodes = normalizeMindMapMaxNodes(options.maxNodes);
    setGeneratingActionCode(action.code);
    setStudioError("");
    try {
      const artifact = await generateStudioArtifact(activeDataset.id, {
        actionCode: action.code,
        topic,
        maxNodes,
        answerModelRouteId: selectedRuntimeRouteId
      });
      setStudioArtifacts((current) => [
        artifact,
        ...current.filter((item) => item.id !== artifact.id)
      ]);
      return true;
    } catch (error) {
      if (handleAuthRejected(error)) {
        return false;
      }
      setStudioError(error instanceof Error ? error.message : "生成失败");
      return false;
    } finally {
      setGeneratingActionCode(null);
    }
  }

  async function handleSubmitMindMapPrompt(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!mindMapAction) {
      return;
    }
    const generated = await handleGenerateStudioArtifact(mindMapAction, {
      topic: mindMapPrompt,
      maxNodes: mindMapMaxNodes
    });
    if (generated) {
      setMindMapPromptOpen(false);
      setMindMapPrompt("");
      setMindMapMaxNodes(DEFAULT_MIND_MAP_MAX_NODES);
    }
  }

  async function handleCopyMessage(message: ChatFlowMessageResp) {
    try {
      if (!window.navigator.clipboard?.writeText) {
        throw new Error("clipboard unavailable");
      }
      await window.navigator.clipboard.writeText(message.content);
      setCopiedMessageId(message.id);
    } catch {
      setToast({ kind: "error", message: "复制失败，请手动选择文本" });
    }
  }

  const overlays = (
    <>
      <AppToast toast={toast} onClose={() => setToast(null)} />
      <DeleteDatasetDialog
        busy={deleteCandidate ? deletingDatasetId === deleteCandidate.id : false}
        dataset={deleteCandidate}
        onCancel={handleCancelDeleteNotebook}
        onConfirm={() => void handleConfirmDeleteNotebook()}
      />
    </>
  );

  if (!authToken) {
    return (
      <LoginPanel
        activeRoute={currentRoute}
        captcha={captcha}
        loginCaptcha={loginCaptcha}
        loginPassword={loginPassword}
        loginStatus={loginStatus}
        loginUsername={loginUsername}
        loggingIn={loggingIn}
        onCaptchaChange={setLoginCaptcha}
        onLogin={handleLogin}
        onPasswordChange={setLoginPassword}
        onUsernameChange={setLoginUsername}
      />
    );
  }

  if (isModelMode) {
    return (
      <main className="min-h-screen bg-[#eef0fb] text-neutral-950">
        {overlays}
        <TopBar
          activeRoute={currentRoute}
      showCreateNotebook={false}
      title="Novex Chat"
      onCreateNotebook={handleCreateNotebook}
      onLogout={handleLogout}
        />
        <div className="grid min-h-[calc(100vh-88px)] grid-cols-1 gap-4 px-5 pb-4 xl:grid-cols-[minmax(0,1fr)_360px]">
          <section className="flex min-h-[520px] min-w-0 flex-col overflow-hidden rounded-lg bg-white">
            <PanelHeader title="对话">
              <SlidersHorizontal aria-hidden="true" className="h-5 w-5 text-neutral-600" />
              <MoreVertical aria-hidden="true" className="h-5 w-5 text-neutral-600" />
            </PanelHeader>
            <div className="min-h-0 flex-1 overflow-auto px-6 py-5">
              <Conversation emptyText="开始新的模型会话。" messages={messages} />
            </div>
            <div className="px-6 pb-5">
              <div className="mx-auto max-w-5xl rounded-2xl border border-neutral-300 bg-white px-4 py-3 shadow-sm">
                <textarea
                  aria-label="提问或创作内容"
                  className="min-h-20 w-full resize-none bg-transparent text-base leading-6 outline-none"
                  onChange={(event) => setInput(event.target.value)}
                  placeholder="提问或创作内容"
                  rows={3}
                  value={input}
                />
                <div className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-center">
                  <div className="min-w-0 flex-1">
                  <ModelRouteSelect
                    routes={llmRoutes}
                    selectedRouteId={selectedLlmRoute?.routeId ?? ""}
                    purpose="chat"
                    onRouteChange={setSelectedLlmRouteId}
                    wide
                  />
                  </div>
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
              <ComposerStatus error={sendError} status={sendStatus} />
              <div className="mt-3 text-center text-xs text-neutral-500">
                模型输出需要结合业务上下文复核。
              </div>
            </div>
          </section>

          <aside className="flex min-h-[520px] flex-col overflow-hidden rounded-lg bg-white">
            <PanelHeader title="Runtime" />
            <div className="min-h-0 flex-1 overflow-auto p-5">
              <ModelSessionPanel
                activeSession={activeSession}
                latestAssistant={latestAssistant}
                llmRoutes={llmRoutes}
                selectedRouteId={selectedLlmRoute?.routeId ?? ""}
                onRouteChange={setSelectedLlmRouteId}
              />
            </div>
          </aside>
        </div>
      </main>
    );
  }

  if (!notebookOpen || !activeDataset) {
    return (
      <main className="min-h-screen bg-white text-neutral-950">
        {overlays}
        <TopBar
          activeRoute={currentRoute}
          title="NotebookLM"
          onCreateNotebook={handleCreateNotebook}
          onLogout={handleLogout}
        />
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
            {actionStatus ? (
              <div className="rounded-lg border border-rose-200 bg-rose-50 p-4 text-sm text-rose-700">
                {actionStatus}
              </div>
            ) : null}

            {datasets.map((dataset, index) => (
              <article
                className={[
                  "relative min-h-[230px] rounded-lg hover:brightness-[0.98]",
                  notebookColors[index % notebookColors.length]
                ].join(" ")}
                key={dataset.id}
              >
                <button
                  aria-label={`打开 ${dataset.name}`}
                  className="flex min-h-[230px] w-full flex-col rounded-lg p-7 pr-14 text-left"
                  onClick={() => openNotebook(dataset)}
                  type="button"
                >
                  <span className="text-5xl">{notebookIcons[index % notebookIcons.length]}</span>
                  <span className="mt-10 line-clamp-3 text-3xl font-medium leading-tight tracking-normal">
                    {dataset.name}
                  </span>
                  <span className="mt-5 text-base text-neutral-600">
                    {formatChineseDate(dataset.createTime)} · {dataset.documentCount} 个来源
                  </span>
                </button>
                <button
                  aria-label={`删除 ${dataset.name}`}
                  className="absolute right-5 top-5 flex h-9 w-9 items-center justify-center rounded-full text-neutral-600 hover:bg-black/5 disabled:cursor-not-allowed disabled:opacity-50"
                  disabled={deletingDatasetId === dataset.id}
                  onClick={() => handleDeleteNotebook(dataset)}
                  title="删除知识库"
                  type="button"
                >
                  <MoreVertical aria-hidden="true" className="h-5 w-5" />
                </button>
              </article>
            ))}
          </div>
        </section>
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-[#eef0fb] text-[12px] text-neutral-950">
      {overlays}
      <TopBar
        activeRoute={currentRoute}
        compact
        title={activeDataset.name}
        onCreateNotebook={handleCreateNotebook}
        onLogout={handleLogout}
      />
      <div
        className="grid min-h-[calc(100vh-88px)] grid-cols-1 gap-4 px-4 pb-4 xl:h-[calc(100vh-88px)] xl:min-h-0 xl:overflow-hidden xl:grid-cols-[360px_minmax(0,1fr)_360px] xl:items-stretch 2xl:grid-cols-[420px_minmax(0,1fr)_420px]"
        data-testid="knowledge-detail-layout"
      >
        <aside
          className="flex min-h-[520px] flex-col overflow-hidden rounded-lg bg-white xl:h-full xl:min-h-0"
          data-testid="knowledge-sources-panel"
        >
          <PanelHeader title="来源" />
          <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-5" data-testid="knowledge-sources-scroll">
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
            <SourceList
              dataset={activeDataset}
              documentLoading={documentLoading}
              documents={documents}
              parseJobs={Object.values(parseJobs)}
            />
          </div>
        </aside>

        <section
          className="flex min-h-[520px] min-w-0 flex-col overflow-hidden rounded-lg bg-white xl:h-full xl:min-h-0"
          data-testid="knowledge-chat-panel"
        >
          <PanelHeader title="对话">
            <SlidersHorizontal aria-hidden="true" className="h-5 w-5 text-neutral-600" />
            <MoreVertical aria-hidden="true" className="h-5 w-5 text-neutral-600" />
          </PanelHeader>
          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4" data-testid="knowledge-chat-scroll">
            <Conversation messages={messages} />
          </div>
          <div className="px-5 pb-4">
            <div className="relative mx-auto max-w-5xl rounded-2xl border border-neutral-300 bg-white px-3.5 py-2.5 shadow-sm">
              <SkillPicker
                activeIndex={activeSkillIndex}
                open={skillPickerOpen}
                skills={filteredSkills}
                onSelect={selectSkill}
              />
              {selectedSkill ? (
                <div className="mb-2 flex flex-wrap items-center gap-2">
                  <span className="inline-flex max-w-full items-center gap-1.5 rounded-full border border-slate-200 bg-slate-50 px-2.5 py-1 text-xs font-medium text-neutral-700">
                    <Bot aria-hidden="true" className="h-3.5 w-3.5 shrink-0" />
                    <span className="truncate">{selectedSkill.name}</span>
                    <button
                      aria-label={`移除 skill ${selectedSkill.name}`}
                      className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full hover:bg-slate-200"
                      onClick={() => setSelectedSkill(null)}
                      type="button"
                    >
                      <X aria-hidden="true" className="h-3 w-3" />
                    </button>
                  </span>
                </div>
              ) : null}
              <textarea
                aria-label="提问或创作内容"
                className="min-h-12 w-full resize-none bg-transparent text-[13px] leading-5 outline-none"
                onChange={(event) => handleComposerChange(event.target.value)}
                onKeyDown={handleComposerKeyDown}
                placeholder="输入 / 选择 Skills，或直接提问当前知识库"
                rows={2}
                value={input}
              />
              <div className="mt-2 flex flex-col gap-2 sm:flex-row sm:items-center">
                <div className="min-w-0 flex-1">
                  <ModelRouteSelect
                    routes={llmRoutes}
                    selectedRouteId={selectedLlmRoute?.routeId ?? ""}
                    purpose="rag_answer"
                    onRouteChange={setSelectedLlmRouteId}
                    wide
                  />
                </div>
                <span className="text-xs text-neutral-500">{activeDataset.documentCount} 个来源</span>
                <button
                  aria-label="发送消息"
                  className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-neutral-100 text-neutral-800 hover:bg-neutral-200 disabled:opacity-50"
                  disabled={sending}
                  onClick={() => void handleSend()}
                  type="button"
                >
                  <ArrowRight aria-hidden="true" className="h-5 w-5" />
                </button>
              </div>
            </div>
            <ComposerStatus error={sendError} status={sendStatus} />
            <div className="mt-3 text-center text-xs text-neutral-500">
              NotebookLM 提供的内容未必准确，因此请仔细核查回答内容。
            </div>
          </div>
        </section>

        <aside
          className="flex min-h-[520px] flex-col overflow-hidden rounded-lg bg-white xl:h-full xl:min-h-0"
          data-testid="knowledge-studio-panel"
        >
          <PanelHeader title="Studio" />
          <div className="min-h-0 flex-1 overflow-y-auto p-5" data-testid="knowledge-studio-scroll">
            <div className="grid grid-cols-2 gap-3">
              {studioItems.map(([label, className]) => (
                <button
                  className={`flex min-h-20 items-center justify-between rounded-lg p-4 text-left text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-50 ${className}`}
                  aria-label={label}
                  disabled={label === "思维导图" ? !mindMapAction || generatingActionCode !== null : true}
                  key={label}
                  onClick={
                    label === "思维导图" && mindMapAction
                      ? () => setMindMapPromptOpen(true)
                      : undefined
                  }
                  type="button"
                >
                  <span>{label}</span>
                  {generatingActionCode && label === "思维导图" ? (
                    <Loader2 aria-hidden="true" className="h-5 w-5 animate-spin" />
                  ) : (
                    <ChevronRight aria-hidden="true" className="h-5 w-5" />
                  )}
                </button>
              ))}
            </div>
            {studioError ? (
              <div className="mt-4 rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
                {studioError}
              </div>
            ) : null}
            <MindMapArtifactLauncher
              artifact={latestMindMapArtifact}
              loading={studioLoading}
              onOpen={(artifact) => setOpenStudioArtifactId(artifact.id)}
            />
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
      {openStudioArtifact ? (
        <div
          className="fixed inset-0 z-40 flex items-center justify-center bg-black/30 p-4"
          onClick={() => setOpenStudioArtifactId(null)}
        >
          <div
            aria-labelledby="studio-artifact-dialog-title"
            aria-modal="true"
            className="flex max-h-[90vh] w-full max-w-6xl flex-col rounded-lg bg-white shadow-2xl"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
          >
            <div className="flex items-start gap-3 border-b border-slate-200 px-5 py-4">
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-base font-semibold text-neutral-950" id="studio-artifact-dialog-title">
                  {openStudioArtifact.title}
                </h2>
                <div className="mt-1 text-xs text-neutral-500">{openStudioArtifact.createTime}</div>
              </div>
              <button
                aria-label="关闭 Studio 产物"
                className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full hover:bg-slate-100"
                onClick={() => setOpenStudioArtifactId(null)}
                type="button"
              >
                <X aria-hidden="true" className="h-4 w-4" />
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-auto p-5">
              <MindMapArtifactPanel artifact={openStudioArtifact} loading={false} />
            </div>
          </div>
        </div>
      ) : null}
      {mindMapPromptOpen ? (
        <div
          className="fixed inset-0 z-40 flex items-center justify-center bg-black/30 p-4"
          onClick={() => setMindMapPromptOpen(false)}
        >
          <form
            aria-labelledby="mind-map-prompt-dialog-title"
            aria-modal="true"
            className="w-full max-w-xl rounded-lg bg-white p-5 shadow-2xl"
            onClick={(event) => event.stopPropagation()}
            onSubmit={handleSubmitMindMapPrompt}
            role="dialog"
          >
            <div className="flex items-start justify-between gap-4">
              <div>
                <h2 className="text-base font-semibold text-neutral-950" id="mind-map-prompt-dialog-title">
                  生成思维导图
                </h2>
                <p className="mt-1 text-sm text-neutral-500">
                  输入你希望总结的方向，系统会结合知识库内容生成完整结构。
                </p>
              </div>
              <button
                aria-label="关闭"
                className="rounded-full p-1 text-neutral-500 hover:bg-neutral-100 hover:text-neutral-900"
                onClick={() => setMindMapPromptOpen(false)}
                type="button"
              >
                <X aria-hidden="true" className="h-5 w-5" />
              </button>
            </div>
            <div className="mt-5 grid gap-4">
              <label className="grid gap-2 text-sm font-medium text-neutral-800" htmlFor="mind-map-prompt">
                总结方向
                <textarea
                  className="min-h-28 rounded-lg border border-slate-200 px-3 py-2 text-sm font-normal text-neutral-900 outline-none focus:border-fuchsia-300 focus:ring-2 focus:ring-fuchsia-100"
                  id="mind-map-prompt"
                  onChange={(event) => setMindMapPrompt(event.target.value)}
                  placeholder="例如：按研究方法、实验流程、适用边界和关键结论完整总结"
                  value={mindMapPrompt}
                />
              </label>
              <label className="grid gap-2 text-sm font-medium text-neutral-800" htmlFor="mind-map-max-nodes">
                节点上限
                <input
                  className="h-10 rounded-lg border border-slate-200 px-3 text-sm font-normal text-neutral-900 outline-none focus:border-fuchsia-300 focus:ring-2 focus:ring-fuchsia-100"
                  id="mind-map-max-nodes"
                  max={MAX_MIND_MAP_MAX_NODES}
                  min={MIN_MIND_MAP_MAX_NODES}
                  onChange={(event) => setMindMapMaxNodes(Number(event.target.value))}
                  type="number"
                  value={mindMapMaxNodes}
                />
              </label>
            </div>
            <div className="mt-6 flex justify-end gap-3">
              <button
                className="rounded-full border border-slate-200 px-4 py-2 text-sm font-semibold text-neutral-700 hover:bg-slate-50"
                onClick={() => setMindMapPromptOpen(false)}
                type="button"
              >
                取消
              </button>
              <button
                className="inline-flex items-center gap-2 rounded-full bg-black px-4 py-2 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:opacity-60"
                disabled={generatingActionCode !== null}
                type="submit"
              >
                {generatingActionCode ? <Loader2 aria-hidden="true" className="h-4 w-4 animate-spin" /> : null}
                生成思维导图
              </button>
            </div>
          </form>
        </div>
      ) : null}
    </main>
  );

  function Conversation({
    messages,
    emptyText = "选择左侧来源后，可以直接提问。回答会绑定当前笔记本、保存会话，并返回可追踪引用。"
  }: {
    messages: ChatFlowMessageResp[];
    emptyText?: string;
  }) {
    if (messages.length === 0) {
      return (
        <div className="mx-auto max-w-4xl pt-6 text-base leading-8 text-neutral-800">
          <p>{emptyText}</p>
        </div>
      );
    }

    return (
      <div className="mx-auto max-w-4xl space-y-6">
        {messages.map((message) =>
          message.role === "assistant" ? (
            <AssistantAnswer
              copied={copiedMessageId === message.id}
              key={message.id}
              message={message}
              onCopy={() => void handleCopyMessage(message)}
            />
          ) : (
            <div className="flex justify-end" key={message.id}>
              <div className="flex max-w-[78%] flex-col items-end gap-2">
                <div className="rounded-2xl bg-neutral-900 px-4 py-3 text-sm leading-6 text-white">
                  {message.content}
                </div>
                <MessageCopyButton
                  copied={copiedMessageId === message.id}
                  onCopy={() => void handleCopyMessage(message)}
                />
              </div>
            </div>
          )
        )}
      </div>
    );
  }
}

function TopBar({
  title,
  onCreateNotebook,
  onLogout,
  compact = false,
  showCreateNotebook = true
}: {
  activeRoute?: AppRouteKey;
  title: string;
  onCreateNotebook: () => void;
  onLogout?: () => void;
  compact?: boolean;
  showCreateNotebook?: boolean;
}) {
  return (
    <header className="flex h-[88px] items-center justify-between gap-5 px-6">
      <Link
        aria-label="回到首页"
        className="flex min-w-0 items-center gap-4 rounded-full pr-3 outline-none hover:opacity-80 focus-visible:ring-2 focus-visible:ring-neutral-400"
        href="/knowledge"
      >
        <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-black text-white">
          <LayoutGrid aria-hidden="true" className="h-6 w-6" />
        </div>
        <h1 className={["truncate font-medium tracking-normal", compact ? "text-2xl" : "text-3xl"].join(" ")}>
          {title}
        </h1>
      </Link>
      <div className="flex shrink-0 items-center gap-3">
        {showCreateNotebook ? (
          <button
            className="hidden h-11 items-center gap-2 rounded-full bg-black px-5 text-sm font-semibold text-white md:flex"
            onClick={onCreateNotebook}
            type="button"
          >
            <Plus aria-hidden="true" className="h-4 w-4" />
            创建笔记本
          </button>
        ) : null}
        <button className="hidden h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium md:flex" type="button">
          <Share2 aria-hidden="true" className="h-4 w-4" />
          分享
        </button>
        {onLogout ? (
          <button
            className="flex h-11 items-center gap-2 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50"
            onClick={onLogout}
            type="button"
          >
            <LogOut aria-hidden="true" className="h-4 w-4" />
            退出登录
          </button>
        ) : null}
      </div>
    </header>
  );
}

function LoginPanel({
  activeRoute,
  captcha,
  loginCaptcha,
  loginPassword,
  loginStatus,
  loginUsername,
  loggingIn,
  onCaptchaChange,
  onLogin,
  onPasswordChange,
  onUsernameChange
}: {
  activeRoute?: AppRouteKey;
  captcha: ImageCaptchaResp | null;
  loginCaptcha: string;
  loginPassword: string;
  loginStatus: string;
  loginUsername: string;
  loggingIn: boolean;
  onCaptchaChange: (value: string) => void;
  onLogin: (event: FormEvent<HTMLFormElement>) => void;
  onPasswordChange: (value: string) => void;
  onUsernameChange: (value: string) => void;
}) {
  return (
    <main className="min-h-screen bg-white text-neutral-950">
      <TopBar activeRoute={activeRoute} title="NotebookLM" onCreateNotebook={() => undefined} />
      <div className="mx-auto flex min-h-[calc(100vh-88px)] max-w-[1440px] items-center justify-center px-6 pb-12">
        <form
          className="w-full max-w-sm rounded-lg border border-slate-200 bg-white p-6 shadow-sm"
          onSubmit={onLogin}
        >
          <div className="flex items-center gap-3">
            <div className="flex h-12 w-12 items-center justify-center rounded-full bg-black text-white">
              <LayoutGrid aria-hidden="true" className="h-6 w-6" />
            </div>
            <div className="min-w-0">
              <h1 className="text-xl font-semibold">NotebookLM 登录</h1>
              <div className="mt-1 text-sm text-neutral-500">Novex Chat Workspace</div>
            </div>
          </div>

          <label className="mt-6 block text-sm font-medium text-neutral-700">
            账号
            <input
              className="mt-2 h-11 w-full rounded-lg border border-slate-200 px-3 text-sm outline-none focus:border-neutral-500"
              onChange={(event) => onUsernameChange(event.target.value)}
              value={loginUsername}
            />
          </label>
          <label className="mt-4 block text-sm font-medium text-neutral-700">
            密码
            <input
              className="mt-2 h-11 w-full rounded-lg border border-slate-200 px-3 text-sm outline-none focus:border-neutral-500"
              onChange={(event) => onPasswordChange(event.target.value)}
              type="password"
              value={loginPassword}
            />
          </label>

          {captcha?.isEnabled ? (
            <div className="mt-4 grid grid-cols-[1fr_128px] gap-3">
              <label className="block text-sm font-medium text-neutral-700">
                验证码
                <input
                  className="mt-2 h-11 w-full rounded-lg border border-slate-200 px-3 text-sm outline-none focus:border-neutral-500"
                  onChange={(event) => onCaptchaChange(event.target.value)}
                  value={loginCaptcha}
                />
              </label>
              <div className="mt-7 flex h-11 items-center justify-center overflow-hidden rounded-lg border border-slate-200 bg-slate-50">
                {captcha.img ? <img alt="验证码" className="h-full w-full object-cover" src={captcha.img} /> : null}
              </div>
            </div>
          ) : null}

          <button
            className="mt-6 h-11 w-full rounded-full bg-black text-sm font-semibold text-white disabled:opacity-50"
            disabled={loggingIn}
            type="submit"
          >
            登录
          </button>
          {loginStatus ? (
            <div className="mt-3 rounded-lg bg-rose-50 px-3 py-2 text-sm text-rose-700">{loginStatus}</div>
          ) : null}
        </form>
      </div>
    </main>
  );
}

function AppToast({
  onClose,
  toast
}: {
  onClose: () => void;
  toast: ToastState | null;
}) {
  if (!toast) {
    return null;
  }

  const isError = toast.kind === "error";
  return (
    <div className="fixed right-5 top-5 z-50 w-[min(360px,calc(100vw-40px))]" role="status">
      <div
        className={[
          "flex items-start gap-3 rounded-lg border bg-white p-4 text-sm shadow-lg",
          isError ? "border-rose-200 text-rose-900" : "border-emerald-200 text-emerald-900"
        ].join(" ")}
      >
        <span
          className={[
            "mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full",
            isError ? "bg-rose-50 text-rose-600" : "bg-emerald-50 text-emerald-600"
          ].join(" ")}
        >
          {isError ? (
            <AlertTriangle aria-hidden="true" className="h-4 w-4" />
          ) : (
            <Check aria-hidden="true" className="h-4 w-4" />
          )}
        </span>
        <div className="min-w-0 flex-1 leading-6">{toast.message}</div>
        <button
          aria-label="关闭提示"
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-neutral-500 hover:bg-neutral-100"
          onClick={onClose}
          type="button"
        >
          <X aria-hidden="true" className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}

function DeleteDatasetDialog({
  busy,
  dataset,
  onCancel,
  onConfirm
}: {
  busy: boolean;
  dataset: DatasetResp | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  if (!dataset) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/35 px-4 py-6">
      <div
        aria-labelledby="delete-dataset-title"
        aria-modal="true"
        className="w-full max-w-md rounded-lg bg-white p-5 shadow-2xl"
        role="dialog"
      >
        <div className="flex items-start gap-3">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-rose-50 text-rose-600">
            <AlertTriangle aria-hidden="true" className="h-5 w-5" />
          </span>
          <div className="min-w-0 flex-1">
            <h2 className="text-lg font-semibold" id="delete-dataset-title">
              确认删除知识库
            </h2>
            <p className="mt-2 text-sm leading-6 text-neutral-600">
              删除「{dataset.name}」会同时删除相关来源、chunks、解析任务、进行中的任务和问答记录。
            </p>
          </div>
        </div>
        <div className="mt-6 flex justify-end gap-3">
          <button
            className="h-10 rounded-full border border-slate-200 px-4 text-sm font-medium hover:bg-slate-50 disabled:opacity-50"
            disabled={busy}
            onClick={onCancel}
            type="button"
          >
            取消
          </button>
          <button
            className="h-10 rounded-full bg-rose-600 px-4 text-sm font-semibold text-white hover:bg-rose-700 disabled:opacity-50"
            disabled={busy}
            onClick={onConfirm}
            type="button"
          >
            {busy ? "删除中..." : "确认删除"}
          </button>
        </div>
      </div>
    </div>
  );
}

function PanelHeader({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="flex h-12 items-center justify-between border-b border-slate-200 px-4">
      <h2 className="text-sm font-semibold">{title}</h2>
      <div className="flex items-center gap-4">{children ?? <PanelLeft aria-hidden="true" className="h-5 w-5 text-neutral-600" />}</div>
    </div>
  );
}

function SourceSearch() {
  return (
    <div className="rounded-2xl border border-slate-200 p-3">
      <div className="text-sm text-neutral-500">在网络中搜索新来源</div>
      <div className="mt-2 flex items-center gap-2">
        <span className="rounded-full border border-slate-200 px-2.5 py-1.5 text-xs font-semibold">Web</span>
        <span className="rounded-full border border-slate-200 px-2.5 py-1.5 text-xs font-semibold">Fast Research</span>
        <button className="ml-auto flex h-8 w-8 items-center justify-center rounded-full bg-neutral-100" type="button">
          <Upload aria-hidden="true" className="h-4 w-4 text-neutral-600" />
        </button>
      </div>
    </div>
  );
}

function SourceList({
  dataset,
  documents,
  documentLoading,
  parseJobs
}: {
  dataset: DatasetResp;
  documents: DocumentResp[];
  documentLoading: boolean;
  parseJobs: ParserJobResp[];
}) {
  const documentIds = new Set(documents.map((document) => document.id));
  const pendingJobs = parseJobs.filter((job) => !documentIds.has(job.documentId));

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between text-sm text-neutral-700">
        <span className="font-medium">已选择 {dataset.documentCount} 个来源</span>
        <span>全选</span>
      </div>
      {documentLoading ? (
        <div className="rounded-lg border border-slate-200 bg-slate-50 p-4 text-sm text-neutral-500">
          正在加载来源...
        </div>
      ) : null}
      {documents.map((document) => (
        <div className="flex items-center gap-3 rounded-lg px-2 py-2 hover:bg-slate-50" key={document.id}>
          <FileText aria-hidden="true" className="h-5 w-5 shrink-0 text-blue-700" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium">{document.name}</div>
            <div className="mt-1 flex flex-wrap gap-x-2 gap-y-1 text-xs text-neutral-500">
              <span>{document.contentType || "document"}</span>
              <span>{documentStatus(document)}</span>
            </div>
          </div>
          {document.ingestionStatus === 4 ? (
            <Check aria-hidden="true" className="h-5 w-5 text-neutral-500" />
          ) : null}
        </div>
      ))}
      {pendingJobs.map((job) => (
        <div className="flex items-center gap-3 rounded-lg px-2 py-2" key={job.id}>
          <FileText aria-hidden="true" className="h-5 w-5 shrink-0 text-amber-600" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium">{job.documentName}</div>
            <div className="mt-1 text-xs text-neutral-500">{parseJobStatus(job)}</div>
          </div>
        </div>
      ))}
      {!documentLoading && documents.length === 0 && pendingJobs.length === 0 ? (
        <div className="rounded-lg border border-dashed border-slate-200 p-4 text-sm leading-6 text-neutral-500">
          还没有上传来源。添加文件后会进入 RAG 解析和索引。
        </div>
      ) : null}
    </div>
  );
}

function ComposerStatus({ error, status }: { error: string; status: string }) {
  if (!error && !status) {
    return null;
  }
  return (
    <div
      className={[
        "mx-auto mt-3 max-w-5xl rounded-lg px-3 py-2 text-sm",
        error ? "bg-rose-50 text-rose-700" : "bg-slate-50 text-neutral-600"
      ].join(" ")}
    >
      {error || status}
    </div>
  );
}

function SkillPicker({
  activeIndex,
  open,
  skills,
  onSelect
}: {
  activeIndex: number;
  open: boolean;
  skills: CapabilityItemResp[];
  onSelect: (skill: CapabilityItemResp) => void;
}) {
  if (!open) {
    return null;
  }

  return (
    <div
      aria-label="Skills"
      className="absolute bottom-full left-0 z-20 mb-2 max-h-64 w-full overflow-y-auto rounded-lg border border-slate-200 bg-white p-2 text-left shadow-xl"
      role="listbox"
    >
      {skills.length === 0 ? (
        <div className="px-3 py-2 text-xs text-neutral-500">没有匹配的 skill</div>
      ) : (
        skills.map((skill, index) => (
          <button
            aria-selected={index === activeIndex}
            className={[
              "flex w-full items-start gap-2 rounded-md px-3 py-2 text-left",
              index === activeIndex ? "bg-neutral-100" : "hover:bg-slate-50"
            ].join(" ")}
            key={skill.code}
            onClick={() => onSelect(skill)}
            role="option"
            type="button"
          >
            <Bot aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-neutral-600" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-xs font-semibold text-neutral-950">{skill.name}</span>
              <span className="mt-0.5 line-clamp-2 block text-xs leading-5 text-neutral-500">
                {skill.description || skill.code}
              </span>
            </span>
            <span className="shrink-0 rounded-full bg-slate-100 px-2 py-0.5 text-[11px] text-neutral-500">
              {skill.code}
            </span>
          </button>
        ))
      )}
    </div>
  );
}

function MarkdownContent({ content }: { content: string }) {
  return (
    <ReactMarkdown
      components={{
        a: ({ children, href }) => (
          <a className="font-medium text-blue-700 underline underline-offset-2" href={href} rel="noreferrer" target="_blank">
            {children}
          </a>
        ),
        blockquote: ({ children }) => (
          <blockquote className="my-3 border-l-4 border-slate-200 pl-3 text-neutral-600">{children}</blockquote>
        ),
        code: ({ children, className }) => (
          <code className={[className, "rounded bg-slate-100 px-1 py-0.5 font-mono text-[0.92em]"].filter(Boolean).join(" ")}>
            {children}
          </code>
        ),
        h1: ({ children }) => <h1 className="mb-2 mt-1 text-lg font-semibold leading-7 text-neutral-950">{children}</h1>,
        h2: ({ children }) => <h2 className="mb-2 mt-1 text-base font-semibold leading-6 text-neutral-950">{children}</h2>,
        h3: ({ children }) => <h3 className="mb-2 mt-3 text-sm font-semibold leading-6 text-neutral-950">{children}</h3>,
        li: ({ children }) => <li className="pl-1">{children}</li>,
        ol: ({ children }) => <ol className="my-3 list-decimal space-y-1 pl-5">{children}</ol>,
        p: ({ children }) => <p className="my-2">{children}</p>,
        pre: ({ children }) => (
          <pre className="my-3 max-w-full overflow-x-auto rounded-lg border border-slate-200 bg-slate-50 p-3 text-xs leading-5">
            {children}
          </pre>
        ),
        table: ({ children }) => (
          <div className="my-3 max-w-full overflow-x-auto rounded-lg border border-slate-200">
            <table className="w-full min-w-max border-collapse text-left text-xs">{children}</table>
          </div>
        ),
        tbody: ({ children }) => <tbody className="divide-y divide-slate-100">{children}</tbody>,
        td: ({ children }) => <td className="border-r border-slate-100 px-3 py-2 align-top last:border-r-0">{children}</td>,
        th: ({ children }) => <th className="border-r border-slate-200 bg-slate-50 px-3 py-2 font-semibold last:border-r-0">{children}</th>,
        thead: ({ children }) => <thead className="border-b border-slate-200">{children}</thead>,
        ul: ({ children }) => <ul className="my-3 list-disc space-y-1 pl-5">{children}</ul>
      }}
      remarkPlugins={[remarkGfm]}
    >
      {content}
    </ReactMarkdown>
  );
}

function AssistantAnswer({
  copied,
  message,
  onCopy
}: {
  copied: boolean;
  message: ChatFlowMessageResp;
  onCopy: () => void;
}) {
  const retrievalHitCount = Number(message.metadata.retrievalHitCount ?? 0);
  const answerStrategy = String(message.metadata.answerStrategy ?? "rag");
  const isModelAnswer = !message.ragTraceId;
  const answerModelRoute = message.routeId ?? metadataString(message.metadata.answerModelRoute);
  const answerModel = message.model ?? metadataString(message.metadata.answerModel);

  return (
    <article className="text-[13px] leading-6 text-neutral-800">
      <div className="flex items-start gap-3">
        <Bot aria-hidden="true" className="mt-1 h-5 w-5 shrink-0 text-neutral-600" />
        <div className="min-w-0 flex-1">
          <MarkdownContent content={message.content} />
          {isModelAnswer ? (
            <div className="mt-3 flex flex-wrap gap-2 text-sm text-neutral-500">
              {message.routeId ? <span>{message.routeId}</span> : null}
              {message.routeId && message.model ? <span>·</span> : null}
              {message.model ? <span>{message.model}</span> : null}
              {message.tokenCount > 0 ? (
                <>
                  {(message.routeId || message.model) ? <span>·</span> : null}
                  <span>{message.tokenCount} tokens</span>
                </>
              ) : null}
            </div>
          ) : (
            <div className="mt-3 flex flex-wrap gap-2 text-sm text-neutral-500">
              <span>Trace #{message.ragTraceId ?? 0}</span>
              <span>·</span>
              <span>{retrievalHitCount} hits</span>
              <span>·</span>
              <span>{answerStrategy}</span>
              {answerModelRoute ? (
                <>
                  <span>·</span>
                  <span>{answerModelRoute}</span>
                </>
              ) : null}
              {answerModel ? (
                <>
                  <span>·</span>
                  <span>{answerModel}</span>
                </>
              ) : null}
            </div>
          )}
          <CitationList citations={message.citations} />
          <div className="mt-4">
            <MessageCopyButton copied={copied} onCopy={onCopy} />
          </div>
        </div>
      </div>
    </article>
  );
}

function MessageCopyButton({ copied, onCopy }: { copied: boolean; onCopy: () => void }) {
  return (
    <div className="flex items-center gap-2 text-xs text-neutral-500">
      <button
        aria-label="复制文本"
        className="flex h-8 items-center gap-1.5 rounded-full border border-slate-200 bg-white px-3 text-xs font-medium text-neutral-600 hover:bg-slate-50"
        onClick={onCopy}
        type="button"
      >
        <Copy aria-hidden="true" className="h-3.5 w-3.5" />
        <span>复制文本</span>
      </button>
      {copied ? <span>已复制</span> : null}
    </div>
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

function normalizeDatasetId(value: number | null | undefined) {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 ? value : null;
}

function normalizeMindMapMaxNodes(value: number | null | undefined) {
  const numeric = typeof value === "number" && Number.isFinite(value) ? Math.trunc(value) : DEFAULT_MIND_MAP_MAX_NODES;
  return Math.min(MAX_MIND_MAP_MAX_NODES, Math.max(MIN_MIND_MAP_MAX_NODES, numeric));
}

function datasetDetailHref(datasetId: number, route: AppRouteKey) {
  return route === "knowledge-sources" ? `/knowledge/sources/${datasetId}` : `/knowledge/${datasetId}`;
}

function purposeRouteId(route: ModelRuntimeRouteSummary, purpose: ModelRoutePurpose) {
  return route.purposeRouteIds?.[purpose] ?? route.routeId;
}

function modelRouteLabel(route: ModelRuntimeRouteSummary, purpose: ModelRoutePurpose) {
  const model = route.model ?? "LLM";
  return `${model} · ${purposeRouteId(route, purpose)}`;
}

function skillQueryFromInput(value: string) {
  const match = value.match(/^\/([^\s]*)$/);
  return match ? match[1].trim().toLowerCase() : null;
}

function removeSkillSlashTrigger(value: string) {
  return value.replace(/^\/[^\s]*\s*/, "");
}

function filterSkills(skills: CapabilityItemResp[], query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return skills;
  }
  return skills.filter((skill) =>
    [skill.name, skill.code, skill.description].some((value) => value.toLowerCase().includes(normalized))
  );
}

function metadataString(value: unknown) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function isAuthRejectedError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error ?? "");
  return message.includes("401") || message.includes("未授权") || message.includes("重新登录");
}

function ModelSessionPanel({
  activeSession,
  latestAssistant,
  llmRoutes,
  selectedRouteId,
  onRouteChange
}: {
  activeSession: ChatFlowSessionResp | null;
  latestAssistant: ChatFlowMessageResp | null;
  llmRoutes: ModelRuntimeRouteSummary[];
  selectedRouteId: string;
  onRouteChange: (routeId: string) => void;
}) {
  return (
    <div className="space-y-4 text-sm text-neutral-700">
      <div className="rounded-lg border border-slate-200 p-4">
        <div className="font-semibold text-neutral-950">LLM</div>
        <div className="mt-3">
          <ModelRouteSelect
            routes={llmRoutes}
            selectedRouteId={selectedRouteId}
            purpose="chat"
            onRouteChange={onRouteChange}
            wide
          />
        </div>
      </div>
      <div className="rounded-lg border border-slate-200 p-4">
        <div className="font-semibold text-neutral-950">{activeSession?.title ?? "Model Chat"}</div>
        <div className="mt-2 text-neutral-500">{activeSession?.messageCount ?? 0} 条消息</div>
      </div>
      <div className="rounded-lg border border-slate-200 p-4">
        <div className="font-semibold text-neutral-950">Latest Output</div>
        <div className="mt-2 break-words leading-6 text-neutral-500">
          {latestAssistant ? (
            <>
              <div>{latestAssistant.routeId ?? "No route"}</div>
              {latestAssistant.model ? <div>{latestAssistant.model}</div> : null}
              <div>{latestAssistant.tokenCount} tokens</div>
            </>
          ) : (
            "No model output yet."
          )}
        </div>
      </div>
    </div>
  );
}

function ModelRouteSelect({
  routes,
  selectedRouteId,
  purpose,
  onRouteChange,
  wide = false
}: {
  routes: ModelRuntimeRouteSummary[];
  selectedRouteId: string;
  purpose: ModelRoutePurpose;
  onRouteChange: (routeId: string) => void;
  wide?: boolean;
}) {
  if (!routes.length) {
    return (
      <span
        className={[
          "rounded-full border border-slate-200 px-3 py-1.5 text-sm text-neutral-500",
          wide ? "inline-flex w-full justify-center" : "hidden sm:inline"
        ].join(" ")}
      >
        No LLM
      </span>
    );
  }

  return (
    <select
      aria-label="LLM 模型"
      className={[
        "h-9 rounded-full border border-slate-200 bg-white px-3 text-sm text-neutral-700 outline-none focus:border-neutral-500",
        wide ? "w-full" : "hidden max-w-64 sm:inline"
      ].join(" ")}
      onChange={(event) => onRouteChange(event.target.value)}
      value={selectedRouteId}
    >
      {routes.map((route) => (
        <option key={route.routeId} value={route.routeId}>
          {modelRouteLabel(route, purpose)}
        </option>
      ))}
    </select>
  );
}

function MindMapArtifactLauncher({
  artifact,
  loading,
  onOpen
}: {
  artifact: StudioArtifactResp | null;
  loading: boolean;
  onOpen: (artifact: StudioArtifactResp) => void;
}) {
  if (!artifact) {
    return loading ? (
      <div className="mt-5 rounded-md border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-neutral-500">
        正在加载 Studio 产物...
      </div>
    ) : null;
  }

  return (
    <button
      aria-label={`打开 ${artifact.title}`}
      className="mt-5 flex w-full items-start gap-3 rounded-lg border border-slate-200 bg-white p-3 text-left shadow-sm outline-none hover:border-fuchsia-200 hover:bg-fuchsia-50/40 focus-visible:ring-2 focus-visible:ring-fuchsia-300"
      onClick={() => onOpen(artifact)}
      type="button"
    >
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-fuchsia-50 text-fuchsia-700">
        <GitBranch aria-hidden="true" className="h-5 w-5" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-semibold text-neutral-950">{artifact.title}</div>
        <div className="mt-1 text-xs text-neutral-500">{artifact.createTime}</div>
      </div>
      <ChevronRight aria-hidden="true" className="mt-2 h-4 w-4 shrink-0 text-neutral-500" />
    </button>
  );
}

function MindMapArtifactPanel({
  artifact,
  loading
}: {
  artifact: StudioArtifactResp | null;
  loading: boolean;
}) {
  if (!artifact) {
    return loading ? (
      <div className="mt-5 rounded-md border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-neutral-500">
        正在加载 Studio 产物...
      </div>
    ) : null;
  }

  const content = parseMindMapContent(artifact.contentJson);
  if (!content) {
    return (
      <div className="mt-5 rounded-md border border-slate-200 bg-white p-4 text-sm text-neutral-700">
        {artifact.contentText || artifact.title}
      </div>
    );
  }

  const displayCitations = content.citations.slice(0, 12);

  return (
    <section className="mt-5 rounded-lg border border-slate-200 bg-white p-4">
      <div className="flex items-start gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-fuchsia-50 text-fuchsia-700">
          <GitBranch aria-hidden="true" className="h-5 w-5" />
        </div>
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold text-neutral-900">{content.title}</h3>
          <div className="mt-1 text-xs text-neutral-500">
            {artifact.ragTraceId ? `Trace #${artifact.ragTraceId}` : artifact.createTime}
          </div>
        </div>
      </div>

      <MindMapMarkmapCanvas content={content} />

      <div className="mt-4 grid gap-3 text-sm text-neutral-700 md:grid-cols-3">
        <div className="rounded-md bg-slate-50 px-3 py-2">
          <div className="text-xs text-neutral-500">节点</div>
          <div className="mt-1 font-semibold text-neutral-950">{content.nodes.length}</div>
        </div>
        <div className="rounded-md bg-slate-50 px-3 py-2">
          <div className="text-xs text-neutral-500">关系</div>
          <div className="mt-1 font-semibold text-neutral-950">{content.edges.length}</div>
        </div>
        <div className="rounded-md bg-slate-50 px-3 py-2">
          <div className="text-xs text-neutral-500">引用</div>
          <div className="mt-1 font-semibold text-neutral-950">{content.citations.length}</div>
        </div>
      </div>

      {displayCitations.length ? (
        <div className="mt-4 flex flex-wrap gap-2">
          {displayCitations.map((citation) => (
            <span className="rounded-full bg-slate-100 px-2.5 py-1 text-xs text-slate-600" key={citation.id}>
              {mindMapCitationLabel(citation)}
            </span>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function MindMapMarkmapCanvas({ content }: { content: MindMapContent }) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const markmapRef = useRef<Markmap | null>(null);
  const markdown = useMemo(() => mindMapContentToMarkdown(content), [content]);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) {
      return;
    }
    const transformer = new Transformer();
    const { root } = transformer.transform(markdown);
    try {
      if (!markmapRef.current) {
        markmapRef.current = Markmap.create(svg, {
          autoFit: true,
          duration: 0
        });
      }
      markmapRef.current.setData(root);
      void markmapRef.current.fit();
    } catch {
      markmapRef.current = null;
    }
  }, [markdown]);

  return (
    <div
      aria-label={`${content.title} 思维导图`}
      className="mt-4 h-[560px] overflow-hidden rounded-lg border border-slate-200 bg-white"
      data-renderer="markmap"
      data-testid="studio-mind-map-canvas"
      role="group"
    >
      <svg aria-label={`${content.title} 思维导图`} className="h-full w-full" ref={svgRef} role="img" />
    </div>
  );
}

function mindMapContentToMarkdown(content: MindMapContent) {
  const root = content.nodes.find((node) => node.id === "root") ?? content.nodes[0] ?? null;
  if (!root) {
    return `# ${escapeMarkmapLine(content.title || "思维导图")}`;
  }

  const nodeById = new Map(content.nodes.map((node) => [node.id, node]));
  const childrenByParent = new Map<string, MindMapContent["nodes"]>();
  for (const edge of content.edges) {
    const source = nodeById.get(edge.source);
    const target = nodeById.get(edge.target);
    if (!source || !target) {
      continue;
    }
    childrenByParent.set(edge.source, [...(childrenByParent.get(edge.source) ?? []), target]);
  }

  const visited = new Set<string>([root.id]);
  const lines = [`# ${escapeMarkmapLine(markmapNodeLabel(root))}`];
  appendMindMapMarkdownChildren(lines, root.id, childrenByParent, visited, 1);

  for (const node of content.nodes) {
    if (visited.has(node.id)) {
      continue;
    }
    visited.add(node.id);
    appendMindMapMarkdownNode(lines, node, 1);
    appendMindMapMarkdownChildren(lines, node.id, childrenByParent, visited, 2);
  }

  return lines.join("\n");
}

function appendMindMapMarkdownChildren(
  lines: string[],
  nodeId: string,
  childrenByParent: Map<string, MindMapContent["nodes"]>,
  visited: Set<string>,
  depth: number
) {
  for (const child of childrenByParent.get(nodeId) ?? []) {
    if (visited.has(child.id)) {
      continue;
    }
    visited.add(child.id);
    appendMindMapMarkdownNode(lines, child, depth);
    appendMindMapMarkdownChildren(lines, child.id, childrenByParent, visited, depth + 1);
  }
}

function appendMindMapMarkdownNode(lines: string[], node: MindMapContent["nodes"][number], depth: number) {
  const indent = "  ".repeat(depth);
  lines.push(`${indent}- ${escapeMarkmapLine(markmapNodeLabel(node))}`);
  if (node.summary) {
    lines.push(`${indent}  - ${escapeMarkmapLine(node.summary)}`);
  }
  if (node.citationRefs?.length) {
    lines.push(`${indent}  - 引用: ${node.citationRefs.slice(0, 4).map(escapeMarkmapLine).join(", ")}`);
  }
}

function markmapNodeLabel(node: MindMapContent["nodes"][number]) {
  return node.label || node.summary || node.id;
}

function escapeMarkmapLine(value: string) {
  return value.replace(/\s+/g, " ").replace(/[#`*_{}[\]()<>]/g, "").trim();
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

function parseMindMapContent(value: Record<string, unknown>): MindMapContent | null {
  const title = stringValue(value.title);
  const nodes = Array.isArray(value.nodes)
    ? value.nodes.map(parseMindMapNode).filter((node): node is MindMapContent["nodes"][number] => Boolean(node))
    : [];
  const edges = Array.isArray(value.edges)
    ? value.edges.map(parseMindMapEdge).filter((edge): edge is MindMapContent["edges"][number] => Boolean(edge))
    : [];
  const citations = Array.isArray(value.citations)
    ? value.citations
        .map(parseMindMapCitation)
        .filter((citation): citation is MindMapContent["citations"][number] => Boolean(citation))
    : [];

  if (!title || nodes.length === 0) {
    return null;
  }

  return {
    title,
    nodes,
    edges,
    citations
  };
}

function parseMindMapNode(value: unknown): MindMapContent["nodes"][number] | null {
  if (!isRecord(value)) {
    return null;
  }
  const id = stringValue(value.id);
  const label = stringValue(value.label);
  if (!id || !label) {
    return null;
  }
  return {
    id,
    label,
    summary: stringValue(value.summary),
    level: numberValue(value.level),
    citationRefs: stringArrayValue(value.citationRefs)
  };
}

function parseMindMapEdge(value: unknown): MindMapContent["edges"][number] | null {
  if (!isRecord(value)) {
    return null;
  }
  const source = stringValue(value.source);
  const target = stringValue(value.target);
  return source && target ? { source, target } : null;
}

function parseMindMapCitation(value: unknown): MindMapContent["citations"][number] | null {
  if (!isRecord(value)) {
    return null;
  }
  const id = stringValue(value.id);
  const documentId = stringValue(value.documentId);
  const chunkId = stringValue(value.chunkId);
  if (!id || !documentId || !chunkId) {
    return null;
  }
  return {
    id,
    documentId,
    chunkId,
    pageNo: numberValue(value.pageNo) ?? null,
    sectionPath: stringArrayValue(value.sectionPath)
  };
}

function mindMapCitationLabel(citation: MindMapContent["citations"][number]) {
  return citation.pageNo ? `${citation.chunkId} · page ${citation.pageNo}` : citation.chunkId;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function stringValue(value: unknown) {
  return typeof value === "string" ? value : "";
}

function numberValue(value: unknown) {
  return typeof value === "number" ? value : undefined;
}

function stringArrayValue(value: unknown) {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
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

function documentStatus(document: DocumentResp) {
  if (document.parseStatus === 4 || document.ingestionStatus === 5) {
    return "处理失败";
  }
  if (document.ingestionStatus === 4) {
    return `已索引 · ${document.chunkCount} chunks`;
  }
  if (document.parseStatus === 3) {
    return `已解析 · ${document.chunkCount} chunks`;
  }
  if (document.parseStatus === 2) {
    return "解析中";
  }
  return "待解析";
}

function formatChineseDate(value: string) {
  const [date] = value.split(" ");
  const [year, month, day] = date.split("-");
  return `${Number(year)}年${Number(month)}月${Number(day)}日`;
}
