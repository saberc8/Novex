import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ChatPage from "./chat/page";
import { metadata } from "./layout";
import Page from "./page";
import {
  createChatFlowSession,
  listChatFlowMessages,
  listChatFlowSessions,
  sendChatFlowMessage
} from "@/api/chat-flow";
import { accountLogin, getImageCaptcha } from "@/api/auth";
import {
  createDataset,
  getParseJob,
  listDatasets,
  submitRagFeedback,
  uploadKnowledgeFile
} from "@/api/knowledge";
import type { ChatFlowMessageResp, ChatFlowSessionResp } from "@/types/chat-flow";
import type { DatasetResp, ParserJobResp } from "@/types/knowledge";

vi.mock("@/api/chat-flow", () => ({
  createChatFlowSession: vi.fn(),
  listChatFlowMessages: vi.fn(),
  listChatFlowSessions: vi.fn(),
  sendChatFlowMessage: vi.fn()
}));

vi.mock("@/api/auth", () => ({
  accountLogin: vi.fn(),
  getImageCaptcha: vi.fn()
}));

vi.mock("@/api/knowledge", () => ({
  createDataset: vi.fn(),
  getParseJob: vi.fn(),
  listDatasets: vi.fn(),
  submitRagFeedback: vi.fn(),
  uploadKnowledgeFile: vi.fn()
}));

const accountLoginMock = vi.mocked(accountLogin);
const getImageCaptchaMock = vi.mocked(getImageCaptcha);
const createChatFlowSessionMock = vi.mocked(createChatFlowSession);
const listChatFlowMessagesMock = vi.mocked(listChatFlowMessages);
const listChatFlowSessionsMock = vi.mocked(listChatFlowSessions);
const sendChatFlowMessageMock = vi.mocked(sendChatFlowMessage);
const createDatasetMock = vi.mocked(createDataset);
const getParseJobMock = vi.mocked(getParseJob);
const listDatasetsMock = vi.mocked(listDatasets);
const submitRagFeedbackMock = vi.mocked(submitRagFeedback);
const uploadKnowledgeFileMock = vi.mocked(uploadKnowledgeFile);

function dataset(overrides: Partial<DatasetResp> = {}): DatasetResp {
  return {
    id: 10,
    tenantId: 1,
    name: "Machine Learning Tools for Environmental Microplastic Analysis",
    description: "Microplastic papers and notes",
    ownerId: 1,
    visibility: 1,
    status: 1,
    retrievalMode: 3,
    documentCount: 7,
    chunkCount: 128,
    createUserString: "admin",
    createTime: "2026-01-30 10:00:00",
    updateUserString: "",
    updateTime: "",
    ...overrides
  };
}

function session(overrides: Partial<ChatFlowSessionResp> = {}): ChatFlowSessionResp {
  return {
    id: 501,
    tenantId: 1,
    appCode: "chat-web",
    mode: "knowledge",
    datasetId: 10,
    title: "Machine Learning Tools for Environmental Microplastic Analysis",
    status: 1,
    routeId: "novex-rag",
    model: null,
    messageCount: 2,
    lastMessagePreview: "Use the current handbook.",
    metadata: {},
    createTime: "2026-01-30 10:00:00",
    updateTime: "2026-01-30 10:01:00",
    ...overrides
  };
}

function assistantMessage(overrides: Partial<ChatFlowMessageResp> = {}): ChatFlowMessageResp {
  return {
    id: 902,
    tenantId: 1,
    sessionId: 501,
    role: "assistant",
    content: "Use the cited source list before drawing conclusions.",
    routeId: "novex-rag",
    model: null,
    ragTraceId: 42,
    citations: [
      {
        documentId: "20",
        chunkId: "20:0",
        pageNo: 3,
        sectionPath: ["Policy"]
      }
    ],
    tokenCount: 9,
    metadata: {
      answerStrategy: "extractive",
      retrievalHitCount: 1
    },
    createTime: "2026-01-30 10:01:00",
    ...overrides
  };
}

function parserJob(overrides: Partial<ParserJobResp> = {}): ParserJobResp {
  return {
    id: 77,
    tenantId: 1,
    datasetId: 10,
    documentId: 20,
    jobType: 2,
    status: 3,
    attemptCount: 1,
    errorMessage: "",
    resultSummary: { parser: "novex-rag-local-structured" },
    documentName: "microplastics.md",
    sourceUri: "file://microplastics.md",
    fileId: 33,
    contentType: "text/markdown",
    parseStatus: 3,
    ingestionStatus: 4,
    chunkCount: 12,
    parserRequest: null,
    createUserString: "admin",
    createTime: "2026-01-30 10:00:00",
    updateUserString: "admin",
    updateTime: "2026-01-30 10:01:00",
    ...overrides
  };
}

describe("Chat web page", () => {
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
    listChatFlowSessionsMock.mockResolvedValue([session()]);
    listChatFlowMessagesMock.mockResolvedValue([]);
    createChatFlowSessionMock.mockResolvedValue(session());
    sendChatFlowMessageMock.mockResolvedValue({
      session: session(),
      userMessage: {
        ...assistantMessage({
          id: 901,
          role: "user",
          content: "Which source should I trust?",
          ragTraceId: null,
          citations: []
        })
      },
      assistantMessage: assistantMessage()
    });
    createDatasetMock.mockResolvedValue(12);
    uploadKnowledgeFileMock.mockResolvedValue({
      file: {
        id: 33,
        name: "microplastics.md",
        originalName: "microplastics.md",
        size: 128,
        url: "",
        parentPath: "/knowledge",
        path: "/knowledge/microplastics.md",
        sha256: "hash",
        contentType: "text/markdown",
        metadata: "",
        thumbnailSize: 0,
        thumbnailName: "",
        thumbnailMetadata: "",
        thumbnailUrl: "",
        extension: "md",
        type: 1,
        storageId: 1,
        storageName: "local",
        createUserString: "admin",
        createTime: "2026-01-30 10:00:00",
        updateUserString: "",
        updateTime: ""
      },
      parseJob: parserJob()
    });
    getParseJobMock.mockResolvedValue(parserJob());
    submitRagFeedbackMock.mockResolvedValue({
      id: 99,
      traceId: 42,
      rating: "citation_issue"
    });
  });

  it("renders a NotebookLM-style notebook grid", async () => {
    render(<Page />);

    expect(screen.getByRole("heading", { name: "NotebookLM", level: 1 })).toBeTruthy();
    expect(screen.getByRole("button", { name: "新建笔记本" })).toBeTruthy();
    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 50 }));
    expect(await screen.findByText("Machine Learning Tools for Environmental Microplastic Analysis")).toBeTruthy();
    expect(await screen.findByText("2026年1月30日 · 7 个来源")).toBeTruthy();
  });

  it("requires login before loading notebooks and stores the token", async () => {
    window.localStorage.clear();
    render(<Page />);

    expect(screen.getByRole("heading", { name: "NotebookLM 登录", level: 1 })).toBeTruthy();
    expect(await screen.findByRole("button", { name: "登录" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "登录" }));

    await waitFor(() =>
      expect(accountLoginMock).toHaveBeenCalledWith({
        username: "admin",
        password: "admin123",
        authType: "ACCOUNT",
        clientId: "novex-chat-web"
      })
    );
    expect(window.localStorage.getItem("novex_token")).toBe("token-2");
    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 50 }));
  });

  it("uses shared chat template metadata", () => {
    expect(metadata.title).toBe("Novex Chat");
    expect(metadata.description).toContain("model and knowledge");
  });

  it("creates a notebook dataset from the app grid", async () => {
    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "新建笔记本" }));

    await waitFor(() =>
      expect(createDatasetMock).toHaveBeenCalledWith({
        name: "未命名的笔记本",
        description: "Created from chat workspace",
        visibility: 1,
        retrievalMode: 3
      })
    );
  });

  it("uploads a source file and polls parser status inside a notebook", async () => {
    render(<Page />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    const file = new File(["# Microplastics\nUse cited sources."], "microplastics.md", {
      type: "text/markdown"
    });
    fireEvent.change(screen.getByLabelText("添加来源"), {
      target: { files: [file] }
    });

    await waitFor(() => expect(uploadKnowledgeFileMock).toHaveBeenCalledWith(10, file));
    await waitFor(() => expect(getParseJobMock).toHaveBeenCalledWith(10, 77));
    expect(await screen.findByText("microplastics.md")).toBeTruthy();
    expect(await screen.findByText("解析完成 · 12 chunks")).toBeTruthy();
  });

  it("asks the selected notebook through chat-flow and renders citations", async () => {
    render(<Page />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    fireEvent.change(screen.getByLabelText("提问或创作内容"), {
      target: { value: "Which source should I trust?" }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    await waitFor(() =>
      expect(sendChatFlowMessageMock).toHaveBeenCalledWith(501, {
        content: "Which source should I trust?",
        limit: 5
      })
    );
    expect(await screen.findByText("Use the cited source list before drawing conclusions.")).toBeTruthy();
    expect(await screen.findByText("Trace #42")).toBeTruthy();
    expect(await screen.findByText("20:0 · page 3")).toBeTruthy();
  });

  it("submits citation feedback for the latest chat-flow answer", async () => {
    render(<Page />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));
    expect(await screen.findByText("Use the cited source list before drawing conclusions.")).toBeTruthy();
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

  it("runs pure model chat on /chat through chat-flow model sessions", async () => {
    const modelSession = session({
      id: 701,
      mode: "model",
      datasetId: null,
      title: "Model Chat",
      routeId: "runtime.llm",
      model: "deepseek-v4-flash",
      lastMessagePreview: "Pure model answer."
    });
    listChatFlowSessionsMock.mockResolvedValue([]);
    createChatFlowSessionMock.mockResolvedValue(modelSession);
    sendChatFlowMessageMock.mockResolvedValue({
      session: {
        ...modelSession,
        messageCount: 2
      },
      userMessage: {
        ...assistantMessage({
          id: 903,
          sessionId: 701,
          role: "user",
          content: "Draft a concise rollout note.",
          routeId: null,
          model: null,
          ragTraceId: null,
          citations: []
        })
      },
      assistantMessage: {
        ...assistantMessage({
          id: 904,
          sessionId: 701,
          content: "Rollout note drafted with the configured chat model.",
          routeId: "runtime.llm",
          model: "deepseek-v4-flash",
          ragTraceId: null,
          citations: [],
          tokenCount: 18,
          metadata: {
            source: "ai.chatFlow.model",
            usage: {
              totalTokens: 18
            }
          }
        })
      }
    });

    render(<ChatPage />);

    await waitFor(() => expect(listChatFlowSessionsMock).toHaveBeenCalledWith({ mode: "model" }));
    fireEvent.change(screen.getByLabelText("提问或创作内容"), {
      target: { value: "Draft a concise rollout note." }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    await waitFor(() =>
      expect(createChatFlowSessionMock).toHaveBeenCalledWith({
        mode: "model",
        title: "Model Chat"
      })
    );
    await waitFor(() =>
      expect(sendChatFlowMessageMock).toHaveBeenCalledWith(701, {
        content: "Draft a concise rollout note."
      })
    );
    expect(await screen.findByText("Rollout note drafted with the configured chat model.")).toBeTruthy();
    expect(await screen.findByText("runtime.llm")).toBeTruthy();
    expect(await screen.findByText("deepseek-v4-flash")).toBeTruthy();
  });
});
