import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ChatPage from "./chat/page";
import KnowledgePage from "./knowledge/page";
import { metadata } from "./layout";
import { mindMapContentToMarkdown } from "@/app-client";
import {
  createChatFlowSession,
  listChatFlowMessages,
  listChatFlowSessions,
  sendChatFlowMessage
} from "@/api/chat-flow";
import { listSkills } from "@/api/capability";
import { accountLogin, getImageCaptcha } from "@/api/auth";
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
  deleteStudioArtifact,
  generateStudioArtifact,
  listDatasetStudioArtifacts,
  listStudioActions
} from "@/api/studio";
import type { ChatFlowMessageResp, ChatFlowSessionResp } from "@/types/chat-flow";
import type { DatasetResp, DocumentResp, ParserJobResp } from "@/types/knowledge";
import type { StudioActionResp, StudioArtifactResp } from "@/types/studio";

const routerPushMock = vi.hoisted(() => vi.fn());

vi.mock("next/navigation", () => ({
  redirect: vi.fn(),
  useRouter: () => ({
    push: routerPushMock
  })
}));

vi.mock("@/api/chat-flow", () => ({
  createChatFlowSession: vi.fn(),
  listChatFlowMessages: vi.fn(),
  listChatFlowSessions: vi.fn(),
  sendChatFlowMessage: vi.fn()
}));

vi.mock("@/api/capability", () => ({
  listSkills: vi.fn()
}));

vi.mock("@/api/auth", () => ({
  accountLogin: vi.fn(),
  getImageCaptcha: vi.fn()
}));

vi.mock("@/api/knowledge", () => ({
  createDataset: vi.fn(),
  deleteDataset: vi.fn(),
  getParseJob: vi.fn(),
  listDocuments: vi.fn(),
  listDatasets: vi.fn(),
  uploadKnowledgeFile: vi.fn()
}));

vi.mock("@/api/model", () => ({
  getModelRuntimeConfig: vi.fn()
}));

vi.mock("@/api/studio", () => ({
  generateStudioArtifact: vi.fn(),
  deleteStudioArtifact: vi.fn(),
  listDatasetStudioArtifacts: vi.fn(),
  listStudioActions: vi.fn()
}));

const accountLoginMock = vi.mocked(accountLogin);
const getImageCaptchaMock = vi.mocked(getImageCaptcha);
const createChatFlowSessionMock = vi.mocked(createChatFlowSession);
const listChatFlowMessagesMock = vi.mocked(listChatFlowMessages);
const listChatFlowSessionsMock = vi.mocked(listChatFlowSessions);
const sendChatFlowMessageMock = vi.mocked(sendChatFlowMessage);
const listSkillsMock = vi.mocked(listSkills);
const createDatasetMock = vi.mocked(createDataset);
const deleteDatasetMock = vi.mocked(deleteDataset);
const getParseJobMock = vi.mocked(getParseJob);
const listDocumentsMock = vi.mocked(listDocuments);
const listDatasetsMock = vi.mocked(listDatasets);
const uploadKnowledgeFileMock = vi.mocked(uploadKnowledgeFile);
const getModelRuntimeConfigMock = vi.mocked(getModelRuntimeConfig);
const generateStudioArtifactMock = vi.mocked(generateStudioArtifact);
const deleteStudioArtifactMock = vi.mocked(deleteStudioArtifact);
const listDatasetStudioArtifactsMock = vi.mocked(listDatasetStudioArtifacts);
const listStudioActionsMock = vi.mocked(listStudioActions);
const writeTextMock = vi.fn();

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

function documentResp(overrides: Partial<DocumentResp> = {}): DocumentResp {
  return {
    id: 30,
    tenantId: 1,
    datasetId: 10,
    name: "architecture.md",
    sourceUri: "file://architecture.md",
    fileId: 33,
    contentType: "text/markdown",
    ownerId: 1,
    visibility: 1,
    parseStatus: 3,
    ingestionStatus: 4,
    chunkCount: 287,
    sourceHash: "hash",
    createUserString: "admin",
    createTime: "2026-01-30 10:00:00",
    updateUserString: "admin",
    updateTime: "2026-01-30 10:01:00",
    ...overrides
  };
}

function studioAction(overrides: Partial<StudioActionResp> = {}): StudioActionResp {
  return {
    id: 3500001,
    tenantId: 1,
    code: "mind_map.generate",
    name: "思维导图",
    description: "Generate a cited mind map from the selected knowledge notebook.",
    surface: "knowledge",
    artifactType: "mind_map",
    pluginCode: "builtin.notebook-studio",
    skillCode: "mind_map",
    permissionCode: "ai:studio:artifact:create",
    modelRoutePolicy: {},
    inputSchema: {},
    outputSchema: {},
    renderer: "mind_map",
    sort: 40,
    status: 1,
    metadata: {},
    createTime: "2026-01-30 10:00:00",
    ...overrides
  };
}

function mindMapArtifact(overrides: Partial<StudioArtifactResp> = {}): StudioArtifactResp {
  return {
    id: 8801,
    tenantId: 1,
    datasetId: 10,
    sessionId: null,
    runId: null,
    ragTraceId: 42,
    actionCode: "mind_map.generate",
    artifactType: "mind_map",
    title: "Training Handbook - 思维导图",
    contentJson: {
      title: "Training Handbook",
      nodes: [
        { id: "root", label: "Training Handbook", level: 0, citationRefs: [] },
        {
          id: "topic-1",
          label: "Security training",
          summary: "Incident response and reporting",
          level: 1,
          citationRefs: ["c1"]
        },
        {
          id: "topic-1-1",
          label: "Response steps",
          summary: "Triage and response workflow",
          level: 2,
          citationRefs: ["c1"]
        },
        {
          id: "topic-1-1-1",
          label: "Escalation path",
          summary: "When incidents should be escalated",
          level: 3,
          citationRefs: ["c3"]
        },
        {
          id: "topic-2",
          label: "Policy basics",
          summary: "Onboarding policy basics",
          level: 1,
          citationRefs: ["c2"]
        }
      ],
      edges: [
        { source: "root", target: "topic-1" },
        { source: "topic-1", target: "topic-1-1" },
        { source: "topic-1-1", target: "topic-1-1-1" },
        { source: "root", target: "topic-2" }
      ],
      citations: [
        {
          id: "c1",
          documentId: "20",
          chunkId: "20:0",
          pageNo: 3,
          sectionPath: ["Policy"]
        },
        {
          id: "c2",
          documentId: "21",
          chunkId: "21:2",
          pageNo: null,
          sectionPath: ["Onboarding"]
        },
        {
          id: "c3",
          documentId: "22",
          chunkId: "22:4",
          pageNo: 8,
          sectionPath: ["Security", "Escalation"]
        }
      ]
    },
    contentText: "Training Handbook\nSecurity training",
    sourceSnapshot: {
      answerModelRoute: "runtime.llm.rag_answer"
    },
    citations: [
      {
        documentId: "20",
        chunkId: "20:0",
        pageNo: 3,
        sectionPath: ["Policy"]
      }
    ],
    version: 1,
    status: 1,
    metadata: {
      renderer: "mind_map"
    },
    createUser: 1,
    createTime: "2026-01-30 10:02:00",
    updateTime: "2026-01-30 10:02:00",
    ...overrides
  };
}

describe("Chat web page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    routerPushMock.mockClear();
    writeTextMock.mockResolvedValue(undefined);
    Object.defineProperty(window.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: writeTextMock
      }
    });
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
    listDocumentsMock.mockResolvedValue({
      list: [documentResp()],
      total: 1
    });
    listStudioActionsMock.mockResolvedValue([studioAction()]);
    listDatasetStudioArtifactsMock.mockResolvedValue([]);
    generateStudioArtifactMock.mockResolvedValue(mindMapArtifact());
    deleteStudioArtifactMock.mockResolvedValue(8801);
    listChatFlowSessionsMock.mockResolvedValue([session()]);
    listChatFlowMessagesMock.mockResolvedValue([]);
    listSkillsMock.mockResolvedValue({
      list: [
        {
          id: 3200002,
          code: "cited_answer",
          name: "Cited Answer",
          description: "RAG question answering skill with grounded citations.",
          kind: "cited_answer",
          status: 1,
          riskLevel: null,
          metadata: {
            promptRules: ["Only answer from the provided knowledge context."]
          },
          createTime: "2026-01-30 10:00:00"
        },
        {
          id: 3200004,
          code: "training_quiz",
          name: "Training Quiz",
          description: "Builds quizzes from cited training content.",
          kind: "training_quiz",
          status: 1,
          riskLevel: null,
          metadata: {},
          createTime: "2026-01-30 10:00:00"
        }
      ],
      total: 2
    });
    getModelRuntimeConfigMock.mockResolvedValue({
      routes: [
        {
          target: "llm",
          routeId: "runtime.llm",
          kind: "llm",
          provider: "deep-seek",
          model: "deepseek-v4-flash",
          baseUrl: "https://api.deepseek.com",
          endpoint: "https://api.deepseek.com/chat/completions",
          maskedApiKey: "sk-****508d",
          purposes: ["chat", "rag_answer"],
          envKeys: ["LLM_API_KEY"],
          purposeRouteIds: {
            chat: "runtime.llm.chat",
            rag_answer: "runtime.llm.rag_answer"
          }
        }
      ],
      missingEnv: []
    });
    createChatFlowSessionMock.mockResolvedValue(session());
    sendChatFlowMessageMock.mockResolvedValue({
      session: session(),
      userMessage: {
        ...assistantMessage({
          id: 901,
          role: "user",
          content: "应该信任哪个来源？",
          ragTraceId: null,
          citations: []
        })
      },
      assistantMessage: assistantMessage({
        routeId: "runtime.llm.rag_answer",
        model: "deepseek-v4-flash",
        metadata: {
          answerStrategy: "llm_grounded",
          retrievalHitCount: 1,
          answerModelRoute: "runtime.llm.rag_answer",
          answerModel: "deepseek-v4-flash"
        }
      })
    });
    createDatasetMock.mockResolvedValue(12);
    deleteDatasetMock.mockResolvedValue(10);
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
  });

  it("renders a NotebookLM-style notebook grid", async () => {
    render(<KnowledgePage />);

    expect(screen.getByRole("heading", { name: "NotebookLM", level: 1 })).toBeTruthy();
    expect(screen.getByRole("button", { name: "新建笔记本" })).toBeTruthy();
    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 50 }));
    expect(await screen.findByText("Machine Learning Tools for Environmental Microplastic Analysis")).toBeTruthy();
    expect(await screen.findByText("2026年1月30日 · 7 个来源")).toBeTruthy();
  });

  it("requires login before loading notebooks and stores the token", async () => {
    window.localStorage.clear();
    render(<KnowledgePage />);

    expect(screen.getByRole("heading", { name: "NotebookLM 登录", level: 1 })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "创建笔记本" })).toBeNull();
    expect(screen.queryByRole("button", { name: "新建笔记本" })).toBeNull();
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

  it("returns to login instead of emptying notebooks when the stored token is rejected", async () => {
    listDatasetsMock.mockRejectedValueOnce(new Error("未授权，请重新登录"));

    render(<KnowledgePage />);

    await waitFor(() => expect(listDatasetsMock).toHaveBeenCalledWith({ page: 1, size: 50 }));
    await waitFor(() => expect(window.localStorage.getItem("novex_token")).toBeNull());
    expect(screen.getByRole("heading", { name: "NotebookLM 登录", level: 1 })).toBeTruthy();
  });

  it("uses shared chat template metadata", () => {
    expect(metadata.title).toBe("Novex Chat");
    expect(metadata.description).toContain("model and knowledge");
  });

  it("keeps the notebook shell focused on the knowledge workflow", () => {
    render(<KnowledgePage />);

    expect(screen.getByRole("link", { name: "回到首页" }).getAttribute("href")).toBe("/knowledge");
    expect(screen.queryByRole("link", { name: "知识库" })).toBeNull();
    expect(screen.queryByRole("link", { name: "来源" })).toBeNull();
    expect(screen.queryByRole("link", { name: "模型对话" })).toBeNull();
    expect(screen.queryByRole("link", { name: "历史" })).toBeNull();
    expect(screen.queryByRole("link", { name: "设置" })).toBeNull();
    expect(screen.queryByRole("link", { name: "打开设置" })).toBeNull();
    expect(screen.getByRole("button", { name: "退出登录" })).toBeTruthy();
  });

  it("clears the local session when signing out", async () => {
    render(<KnowledgePage />);

    fireEvent.click(screen.getByRole("button", { name: "退出登录" }));

    expect(window.localStorage.getItem("novex_token")).toBeNull();
    expect(await screen.findByRole("heading", { name: "NotebookLM 登录", level: 1 })).toBeTruthy();
  });

  it("creates a notebook dataset from the app grid", async () => {
    render(<KnowledgePage />);

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

  it("creates the next unnamed notebook when the default name already exists", async () => {
    listDatasetsMock.mockResolvedValue({
      list: [
        dataset({
          id: 10,
          name: "未命名的笔记本"
        }),
        dataset({
          id: 11,
          name: "未命名的笔记本 3"
        })
      ],
      total: 2
    });

    render(<KnowledgePage />);

    expect(await screen.findByRole("button", { name: "打开 未命名的笔记本" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "新建笔记本" }));

    await waitFor(() =>
      expect(createDatasetMock).toHaveBeenCalledWith({
        name: "未命名的笔记本 2",
        description: "Created from chat workspace",
        visibility: 1,
        retrievalMode: 3
      })
    );
  });

  it("uploads a source file and polls parser status inside a notebook", async () => {
    render(<KnowledgePage />);

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

  it("loads existing notebook documents into the source list", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));

    await waitFor(() => expect(listDocumentsMock).toHaveBeenCalledWith(10, { page: 1, size: 100 }));
    expect(await screen.findByText("architecture.md")).toBeTruthy();
    expect(await screen.findByText("已索引 · 287 chunks")).toBeTruthy();
  });

  it("pushes the notebook id into the detail route when opening a notebook", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));

    expect(routerPushMock).toHaveBeenCalledWith("/knowledge/10");
  });

  it("keeps notebook detail columns independently scrollable with compact spacing", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));

    const topBar = screen.getByRole("banner");
    expect(topBar.className).toContain("h-16");
    expect(topBar.className).toContain("px-4");
    expect(screen.getByRole("heading", { name: /Machine Learning Tools/ }).className).toContain("text-xl");

    const layout = await screen.findByTestId("knowledge-detail-layout");
    expect(layout.className).toContain("gap-3");
    expect(layout.className).toContain("px-3");
    expect(layout.className).toContain("pb-3");
    expect(layout.className).toContain("xl:h-[calc(100vh-64px)]");
    expect(layout.className).toContain("xl:overflow-hidden");

    for (const testId of ["knowledge-sources-panel", "knowledge-chat-panel", "knowledge-studio-panel"]) {
      const panel = screen.getByTestId(testId);
      expect(panel.className).toContain("min-h-[480px]");
      expect(panel.className).toContain("rounded-md");
    }

    const sourcesHeader = screen.getByRole("heading", { name: "来源" }).parentElement as HTMLElement;
    expect(sourcesHeader.className).toContain("h-10");
    expect(sourcesHeader.className).toContain("px-3");

    for (const testId of ["knowledge-sources-scroll", "knowledge-chat-scroll", "knowledge-studio-scroll"]) {
      const region = screen.getByTestId(testId);
      expect(region.className).toContain("min-h-0");
      expect(region.className).toContain("flex-1");
      expect(region.className).toContain("overflow-y-auto");
    }

    expect(screen.getByTestId("knowledge-sources-scroll").className).toContain("p-4");
    expect(screen.getByTestId("knowledge-chat-scroll").className).toContain("px-4");
    expect(screen.getByTestId("knowledge-chat-scroll").className).toContain("py-3");
    expect(screen.getByTestId("knowledge-studio-scroll").className).toContain("p-4");
    expect((screen.getByText("添加来源").closest("label") as HTMLElement).className).toContain("h-9");
    expect(screen.queryByRole("button", { name: "添加笔记" })).toBeNull();
  });

  it("renders compact Studio action items in the upper grid", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));

    const studioScroll = await screen.findByTestId("knowledge-studio-scroll");
    const actionGrid = studioScroll.firstElementChild as HTMLElement;
    expect(actionGrid.className).toContain("gap-2");

    const mindMapAction = screen.getByRole("button", { name: "思维导图" });
    expect(mindMapAction.className).toContain("min-h-14");
    expect(mindMapAction.className).toContain("p-2.5");
    expect(mindMapAction.className).toContain("text-[11px]");
    expect(mindMapAction.querySelector("svg")?.className.baseVal).toContain("h-3.5");
  });

  it("generates a cited mind map artifact from the Studio panel", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    fireEvent.click(await screen.findByRole("button", { name: "思维导图" }));
    expect(generateStudioArtifactMock).not.toHaveBeenCalled();

    expect(await screen.findByRole("heading", { name: "生成思维导图" })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("总结方向"), {
      target: { value: "按研究方法、实验流程、适用边界完整总结" }
    });
    fireEvent.change(screen.getByLabelText("节点上限"), {
      target: { value: "72" }
    });
    fireEvent.click(screen.getByRole("button", { name: "生成思维导图" }));

    await waitFor(() =>
      expect(generateStudioArtifactMock).toHaveBeenCalledWith(10, {
        actionCode: "mind_map.generate",
        topic: "按研究方法、实验流程、适用边界完整总结",
        maxNodes: 72,
        answerModelRouteId: "runtime.llm.rag_answer"
      })
    );
    expect(await screen.findByRole("button", { name: "预览 Training Handbook - 思维导图" })).toBeTruthy();
    expect(await screen.findByText("2026-01-30 10:02:00")).toBeTruthy();
    expect(screen.queryByText("Security training")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "预览 Training Handbook - 思维导图" }));

    const artifactDialog = await screen.findByRole("dialog", { name: "Training Handbook - 思维导图" });
    expect(artifactDialog).toBeTruthy();
    expect(artifactDialog.textContent).toContain("2026-01-30 10:02:00");
    expect(artifactDialog.textContent).toContain("节点");
    expect(artifactDialog.textContent).toContain("关系");
    expect(artifactDialog.textContent).toContain("引用");
    expect(artifactDialog.textContent).toContain("20:0 · page 3");

    const canvas = await screen.findByTestId("studio-mind-map-canvas");
    expect(canvas.getAttribute("data-renderer")).toBe("markmap");
    expect(canvas.className).toContain("overflow-hidden");
    expect(canvas.querySelector("svg")).toBeTruthy();
  });

  it("renders mind map markdown without duplicating summaries or citation refs as nodes", () => {
    const markdown = mindMapContentToMarkdown({
      title: "未命名的笔记本",
      nodes: [
        { id: "root", label: "未命名的笔记本", level: 0, citationRefs: [] },
        {
          id: "topic-1",
          label: "钉钉的象征",
          summary: "钉三多源于雨燕，象征持续飞行与不落地。",
          level: 1,
          citationRefs: ["c1"]
        },
        {
          id: "topic-1-point-1",
          label: "特性",
          summary: "雨燕可连续飞行300多天。",
          level: 2,
          citationRefs: ["c1"]
        },
        {
          id: "topic-1-point-1-detail-1",
          label: "雨燕可连续飞行300多天",
          summary: "雨燕可连续飞行300多天。",
          level: 3,
          citationRefs: ["c1"]
        }
      ],
      edges: [
        { source: "root", target: "topic-1" },
        { source: "topic-1", target: "topic-1-point-1" },
        { source: "topic-1-point-1", target: "topic-1-point-1-detail-1" }
      ],
      citations: [
        {
          id: "c1",
          documentId: "20",
          chunkId: "20:0",
          pageNo: 3,
          sectionPath: ["Policy"]
        }
      ]
    });

    expect(markdown.match(/雨燕可连续飞行300多天/g) ?? []).toHaveLength(1);
    expect(markdown).not.toContain("引用:");
    expect(markdown).not.toContain("钉三多源于雨燕");
  });

  it("keeps generated Studio artifacts in a lower preview/delete list", async () => {
    const existingArtifact = mindMapArtifact({
      id: 8701,
      title: "已有分析 - 思维导图",
      createTime: "2026-01-30 09:40:00"
    });
    const firstGenerated = mindMapArtifact({
      id: 8801,
      title: "第一次生成 - 思维导图",
      createTime: "2026-01-30 10:02:00"
    });
    const secondGenerated = mindMapArtifact({
      id: 8802,
      title: "第二次生成 - 思维导图",
      createTime: "2026-01-30 10:05:00"
    });
    listDatasetStudioArtifactsMock.mockResolvedValueOnce([existingArtifact]);
    generateStudioArtifactMock
      .mockResolvedValueOnce(firstGenerated)
      .mockResolvedValueOnce(secondGenerated);
    deleteStudioArtifactMock.mockResolvedValueOnce(firstGenerated.id);

    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    expect(await screen.findByText("生成内容")).toBeTruthy();
    expect(await screen.findByRole("button", { name: "预览 已有分析 - 思维导图" })).toBeTruthy();
    expect(screen.queryByText("当前笔记本")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "思维导图" }));
    fireEvent.change(await screen.findByLabelText("总结方向"), {
      target: { value: "第一次总结方向" }
    });
    fireEvent.click(screen.getByRole("button", { name: "生成思维导图" }));
    expect(await screen.findByRole("button", { name: "预览 第一次生成 - 思维导图" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "预览 已有分析 - 思维导图" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "思维导图" }));
    fireEvent.change(await screen.findByLabelText("总结方向"), {
      target: { value: "第二次总结方向" }
    });
    fireEvent.click(screen.getByRole("button", { name: "生成思维导图" }));
    expect(await screen.findByRole("button", { name: "预览 第二次生成 - 思维导图" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "预览 第一次生成 - 思维导图" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "预览 已有分析 - 思维导图" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "删除 第一次生成 - 思维导图" }));
    await waitFor(() => expect(deleteStudioArtifactMock).toHaveBeenCalledWith(firstGenerated.id));
    expect(screen.queryByRole("button", { name: "预览 第一次生成 - 思维导图" })).toBeNull();
    expect(screen.getByRole("button", { name: "预览 第二次生成 - 思维导图" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "预览 已有分析 - 思维导图" })).toBeTruthy();
  });

  it("shows a Studio loading failure instead of a silent disabled action", async () => {
    listStudioActionsMock.mockRejectedValueOnce(new Error("请求的资源不存在"));
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));

    expect(await screen.findByText("Studio 功能加载失败：请求的资源不存在")).toBeTruthy();
  });

  it("does not prefill the knowledge chat input with an example question", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));

    expect((screen.getByLabelText("提问或创作内容") as HTMLTextAreaElement).value).toBe("");
  });

  it("uses a compact type scale and composer height in notebook detail", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));

    const layout = await screen.findByTestId("knowledge-detail-layout");
    expect(layout.closest("main")?.className).toContain("text-[11px]");

    const shell = screen.getByTestId("knowledge-composer-shell");
    expect(shell.className).toContain("max-w-2xl");

    const composer = screen.getByLabelText("提问或创作内容") as HTMLTextAreaElement;
    expect(composer.className).toContain("min-h-8");
    expect(composer.className).toContain("text-[11px]");
    expect(composer.getAttribute("rows")).toBe("1");
  });

  it("uses a short custom model selector beside the send button in notebook detail", async () => {
    getModelRuntimeConfigMock.mockResolvedValueOnce({
      routes: [
        {
          target: "llm",
          routeId: "runtime.llm",
          kind: "llm",
          provider: "deep-seek",
          model: "deepseek-v4-flash",
          baseUrl: "https://api.deepseek.com",
          endpoint: "https://api.deepseek.com/chat/completions",
          maskedApiKey: "sk-****508d",
          purposes: ["chat", "rag_answer"],
          envKeys: ["LLM_API_KEY"],
          purposeRouteIds: {
            chat: "runtime.llm.chat",
            rag_answer: "runtime.llm.rag_answer"
          }
        },
        {
          target: "llm",
          routeId: "runtime.llm.qwen",
          kind: "llm",
          provider: "dash-scope",
          model: "qwen-max",
          baseUrl: "https://dashscope.aliyuncs.com",
          endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
          maskedApiKey: "sk-****qwen",
          purposes: ["chat", "rag_answer"],
          envKeys: ["QWEN_API_KEY"],
          purposeRouteIds: {
            chat: "runtime.llm.qwen_chat",
            rag_answer: "runtime.llm.qwen_rag"
          }
        }
      ],
      missingEnv: []
    });

    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));

    expect(screen.queryByRole("combobox", { name: "LLM 模型" })).toBeNull();
    const modelButton = await screen.findByRole("button", { name: "LLM 模型 deepseek-v4-flash" });
    expect(screen.getByRole("button", { name: "发送消息" }).parentElement?.textContent).toContain("deepseek-v4-flash");

    fireEvent.click(modelButton);
    const listbox = await screen.findByRole("listbox", { name: "选择 LLM 模型" });
    expect(listbox.textContent).toContain("deepseek-v4-flash");
    expect(listbox.textContent).toContain("qwen-max");
    expect(listbox.textContent).not.toContain("runtime.llm");

    fireEvent.click(screen.getByRole("option", { name: "qwen-max" }));
    expect(screen.getByRole("button", { name: "LLM 模型 qwen-max" })).toBeTruthy();

    fireEvent.change(screen.getByLabelText("提问或创作内容"), {
      target: { value: "应该信任哪个来源？" }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    await waitFor(() =>
      expect(sendChatFlowMessageMock).toHaveBeenCalledWith(501, {
        content: "应该信任哪个来源？",
        limit: 5,
        answerModelRouteId: "runtime.llm.qwen_rag"
      })
    );
  });

  it("deletes a notebook from the card menu after confirming in the app dialog", async () => {
    const confirmSpy = vi.spyOn(window, "confirm");
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /删除 Machine Learning Tools/ }));
    expect(screen.getByRole("dialog", { name: "确认删除知识库" })).toBeTruthy();
    expect(confirmSpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));

    await waitFor(() => expect(deleteDatasetMock).toHaveBeenCalledWith(10));
    expect(routerPushMock).not.toHaveBeenCalledWith("/knowledge/10");
    await waitFor(() =>
      expect(
        screen.queryByText("Machine Learning Tools for Environmental Microplastic Analysis")
      ).toBeNull()
    );
  });

  it("shows delete failures as a toast instead of a card error", async () => {
    deleteDatasetMock.mockRejectedValueOnce(new Error("系统异常，请稍后重试"));
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /删除 Machine Learning Tools/ }));
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));

    expect((await screen.findByRole("status")).textContent).toContain("系统异常，请稍后重试");
    expect(await screen.findByText("Machine Learning Tools for Environmental Microplastic Analysis")).toBeTruthy();
  });

  it("asks the selected notebook through chat-flow and renders citations", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    fireEvent.change(screen.getByLabelText("提问或创作内容"), {
      target: { value: "应该信任哪个来源？" }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    await waitFor(() =>
      expect(sendChatFlowMessageMock).toHaveBeenCalledWith(501, {
        content: "应该信任哪个来源？",
        limit: 5,
        answerModelRouteId: "runtime.llm.rag_answer"
      })
    );
    expect(await screen.findByText("Use the cited source list before drawing conclusions.")).toBeTruthy();
    expect(await screen.findByText("Trace #42")).toBeTruthy();
    expect(await screen.findByText("runtime.llm.rag_answer")).toBeTruthy();
    expect((await screen.findAllByText("deepseek-v4-flash")).length).toBeGreaterThan(0);
    expect(await screen.findByText("20:0 · page 3")).toBeTruthy();
  });

  it("opens the skill picker with slash and sends the selected skill to chat-flow", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    const composer = screen.getByLabelText("提问或创作内容");
    fireEvent.change(composer, {
      target: { value: "/" }
    });

    expect(await screen.findByRole("listbox", { name: "Skills" })).toBeTruthy();
    expect(await screen.findByRole("option", { name: /Cited Answer/ })).toBeTruthy();
    fireEvent.click(screen.getByRole("option", { name: /Cited Answer/ }));
    expect(screen.getByText("Cited Answer")).toBeTruthy();

    fireEvent.change(composer, {
      target: { value: "总结关键约束" }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    await waitFor(() =>
      expect(sendChatFlowMessageMock).toHaveBeenCalledWith(501, {
        content: "总结关键约束",
        limit: 5,
        answerModelRouteId: "runtime.llm.rag_answer",
        skillCode: "cited_answer"
      })
    );
  });

  it("renders assistant markdown tables in the notebook chat", async () => {
    sendChatFlowMessageMock.mockResolvedValueOnce({
      session: session(),
      userMessage: {
        ...assistantMessage({
          id: 911,
          role: "user",
          content: "列一个对比表",
          ragTraceId: null,
          citations: []
        })
      },
      assistantMessage: assistantMessage({
        id: 912,
        content: [
          "## 来源对比",
          "",
          "| 来源 | 可信度 |",
          "| --- | --- |",
          "| 置身钉内 | 高 |",
          "",
          "- 优先看原文引用",
          "",
          "```ts",
          "const score = 1;",
          "```"
        ].join("\n")
      })
    });
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    fireEvent.change(screen.getByLabelText("提问或创作内容"), {
      target: { value: "列一个对比表" }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    expect(await screen.findByRole("heading", { name: "来源对比", level: 2 })).toBeTruthy();
    expect(await screen.findByRole("table")).toBeTruthy();
    expect(await screen.findByText("置身钉内")).toBeTruthy();
    expect(await screen.findByText("const score = 1;")).toBeTruthy();
  });

  it("shows send failures instead of silently dropping chat requests", async () => {
    sendChatFlowMessageMock.mockRejectedValueOnce(new Error("RAG unavailable"));
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    fireEvent.change(screen.getByLabelText("提问或创作内容"), {
      target: { value: "Why is there no answer?" }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));

    expect(await screen.findByText("RAG unavailable")).toBeTruthy();
  });

  it("copies chat message text and hides unavailable feedback actions", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: /打开 Machine Learning Tools/ }));
    fireEvent.change(screen.getByLabelText("提问或创作内容"), {
      target: { value: "应该信任哪个来源？" }
    });
    fireEvent.click(screen.getByRole("button", { name: "发送消息" }));
    expect(await screen.findByText("Use the cited source list before drawing conclusions.")).toBeTruthy();

    const copyButtons = screen.getAllByRole("button", { name: "复制文本" });
    expect(copyButtons).toHaveLength(2);
    fireEvent.click(copyButtons[1]);

    await waitFor(() =>
      expect(writeTextMock).toHaveBeenCalledWith("Use the cited source list before drawing conclusions.")
    );
    expect(await screen.findByText("已复制")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "有帮助" })).toBeNull();
    expect(screen.queryByRole("button", { name: "答案不准确" })).toBeNull();
    expect(screen.queryByRole("button", { name: "引用问题" })).toBeNull();
    expect(screen.queryByRole("button", { name: "保存到笔记" })).toBeNull();
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
        content: "Draft a concise rollout note.",
        modelRouteId: "runtime.llm.chat"
      })
    );
    expect(await screen.findByText("Rollout note drafted with the configured chat model.")).toBeTruthy();
    expect((await screen.findAllByText("runtime.llm")).length).toBeGreaterThan(0);
    expect((await screen.findAllByText("deepseek-v4-flash")).length).toBeGreaterThan(0);
  });
});
