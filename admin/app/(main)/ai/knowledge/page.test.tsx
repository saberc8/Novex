import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import KnowledgePage from "./page";
import {
  askDataset,
  createDataset,
  listDatasets,
  listDocuments,
  uploadTextDocument
} from "@/api/ai/knowledge";
import { getModelRuntimeConfig } from "@/api/ai/model";
import { generateDatasetArtifact } from "@/api/ai/studio";
import type { DatasetResp, DocumentResp } from "@/types/ai";

vi.mock("@/api/ai/knowledge", () => ({
  askDataset: vi.fn(),
  createDataset: vi.fn(),
  listDatasets: vi.fn(),
  listDocuments: vi.fn(),
  uploadTextDocument: vi.fn()
}));

vi.mock("@/api/ai/model", () => ({
  getModelRuntimeConfig: vi.fn()
}));

vi.mock("@/api/ai/studio", () => ({
  generateDatasetArtifact: vi.fn()
}));

vi.mock("@/components/ai/mind-map-artifact", () => ({
  MindMapArtifact: ({ artifact }: { artifact: { title: string } }) => (
    <div data-testid="mind-map-artifact">{artifact.title}</div>
  )
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

const createDatasetMock = vi.mocked(createDataset);
const listDatasetsMock = vi.mocked(listDatasets);
const listDocumentsMock = vi.mocked(listDocuments);
const uploadTextDocumentMock = vi.mocked(uploadTextDocument);
const askDatasetMock = vi.mocked(askDataset);
const getModelRuntimeConfigMock = vi.mocked(getModelRuntimeConfig);
const generateDatasetArtifactMock = vi.mocked(generateDatasetArtifact);

function dataset(overrides: Partial<DatasetResp>): DatasetResp {
  return {
    id: 10,
    tenantId: 1,
    name: "员工手册",
    description: "制度与培训资料",
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

function document(overrides: Partial<DocumentResp>): DocumentResp {
  return {
    id: 20,
    tenantId: 1,
    datasetId: 10,
    name: "入职流程.pdf",
    sourceUri: "",
    fileId: 100,
    contentType: "application/pdf",
    ownerId: 1,
    visibility: 1,
    parseStatus: 1,
    ingestionStatus: 1,
    chunkCount: 0,
    sourceHash: "",
    createUserString: "admin",
    createTime: "2026-06-05 10:10:00",
    updateUserString: "",
    updateTime: "",
    ...overrides
  };
}

describe("KnowledgePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    createDatasetMock.mockResolvedValue(99);
    listDatasetsMock.mockResolvedValue({
      list: [dataset({ id: 10 }), dataset({ id: 11, name: "产品资料", documentCount: 0 })],
      total: 2
    });
    listDocumentsMock.mockResolvedValue({ list: [document({ id: 20 })], total: 1 });
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
    uploadTextDocumentMock.mockResolvedValue(88);
    askDatasetMock.mockResolvedValue({
      traceId: 42,
      answer: "Training starts on Monday.",
      citations: [
        {
          documentId: "20",
          chunkId: "20:0",
          pageNo: null,
          sectionPath: []
        }
      ],
      retrievalHitCount: 1,
      answerStrategy: "llm_grounded",
      embeddingModelRoute: "runtime.embedding",
      rerankModelRoute: "runtime.reranker",
      answerModelRoute: "runtime.llm.rag_answer",
      answerModel: "deepseek-v4-flash"
    });
    generateDatasetArtifactMock.mockResolvedValue({
      id: 3501,
      tenantId: 1,
      datasetId: 10,
      sessionId: null,
      runId: null,
      ragTraceId: null,
      actionCode: "mind_map.generate",
      artifactType: "mind_map",
      title: "产品定位 - 思维导图",
      contentJson: {
        title: "产品定位",
        nodes: [
          { id: "root", label: "产品定位", summary: "根节点", level: 0, citationRefs: [] },
          { id: "topic-1", label: "市场机会", summary: "机会摘要", level: 1, citationRefs: ["c1"] }
        ],
        edges: [{ source: "root", target: "topic-1" }],
        citations: [{ id: "c1", documentId: "20", chunkId: "20:0", pageNo: null, sectionPath: ["战略"] }]
      },
      contentText: "产品定位\n市场机会",
      sourceSnapshot: {},
      citations: [{ documentId: "20", chunkId: "20:0", pageNo: null, sectionPath: ["战略"] }],
      version: 1,
      status: 1,
      metadata: { renderer: "mind_map" },
      createUser: 1,
      createTime: "2026-06-10 10:00:00",
      updateTime: "2026-06-10 10:00:00"
    });
  });

  it("keeps dataset and document panels aligned and loads documents for the selected dataset", async () => {
    render(<KnowledgePage />);

    const layout = await screen.findByTestId("knowledge-layout");
    expect(layout.className).toContain("items-start");
    expect(screen.getByTestId("dataset-list-panel").className).toContain("self-start");
    expect(screen.getByTestId("documents-panel").className).toContain("self-start");
    expect(screen.getByTestId("documents-panel").className).toContain("content-start");
    expect(await screen.findByTestId("dataset-card-10")).toBeTruthy();
    expect(await screen.findByText("入职流程.pdf")).toBeTruthy();
    await waitFor(() =>
      expect(listDocumentsMock).toHaveBeenCalledWith(10, {
        page: 1,
        size: 20
      })
    );
  });

  it("submits a new dataset with default visibility and retrieval mode", async () => {
    render(<KnowledgePage />);

    fireEvent.click(await screen.findByRole("button", { name: "新增知识库" }));
    fireEvent.change(screen.getByPlaceholderText("知识库名称"), {
      target: { value: "研发制度" }
    });
    fireEvent.change(screen.getByPlaceholderText("描述这个知识库的内容范围"), {
      target: { value: "研发流程与规范" }
    });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() =>
      expect(createDatasetMock).toHaveBeenCalledWith({
        name: "研发制度",
        description: "研发流程与规范",
        visibility: 1,
        retrievalMode: 3
      })
    );
  });

  it("uploads text into the selected dataset", async () => {
    render(<KnowledgePage />);

    await screen.findByTestId("dataset-card-10");
    fireEvent.change(screen.getByPlaceholderText("文档名称"), {
      target: { value: "入职手册.txt" }
    });
    fireEvent.change(screen.getByPlaceholderText("文本或 Markdown"), {
      target: { value: "Training starts on Monday." }
    });
    fireEvent.click(screen.getByRole("button", { name: "上传文档" }));

    await waitFor(() =>
      expect(uploadTextDocumentMock).toHaveBeenCalledWith(10, {
        name: "入职手册.txt",
        content: "Training starts on Monday.",
        contentType: "text/plain"
      })
    );
  });

  it("asks the selected dataset and displays citations", async () => {
    render(<KnowledgePage />);

    await screen.findByTestId("dataset-card-10");
    fireEvent.change(screen.getByPlaceholderText("输入测试问题"), {
      target: { value: "培训什么时候开始？" }
    });
    fireEvent.click(screen.getByRole("button", { name: "提问" }));

    await waitFor(() =>
      expect(askDatasetMock).toHaveBeenCalledWith(10, {
        question: "培训什么时候开始？",
        limit: 5,
        answerModelRouteId: "runtime.llm.rag_answer"
      })
    );
    expect(await screen.findByText("Training starts on Monday.")).toBeTruthy();
    expect(await screen.findByText("20:0")).toBeTruthy();
    expect(await screen.findByText("Trace #42")).toBeTruthy();
    expect(await screen.findByText("runtime.llm.rag_answer")).toBeTruthy();
    expect(await screen.findByText("deepseek-v4-flash")).toBeTruthy();
  });

  it("opens a mind map prompt dialog and generates a rendered artifact", async () => {
    render(<KnowledgePage />);

    await screen.findByTestId("dataset-card-10");
    fireEvent.click(screen.getByRole("button", { name: "思维导图" }));

    expect(await screen.findByRole("heading", { name: "生成思维导图" })).toBeTruthy();
    fireEvent.change(screen.getByPlaceholderText("例如：围绕产品定位、关键矛盾、技术路线、风险和落地路径总结"), {
      target: { value: "围绕产品定位、关键矛盾和落地路径总结" }
    });
    fireEvent.change(screen.getByLabelText("节点上限"), {
      target: { value: "72" }
    });
    fireEvent.click(screen.getByRole("button", { name: "生成" }));

    await waitFor(() =>
      expect(generateDatasetArtifactMock).toHaveBeenCalledWith(10, {
        actionCode: "mind_map.generate",
        topic: "围绕产品定位、关键矛盾和落地路径总结",
        maxNodes: 72,
        answerModelRouteId: "runtime.llm.rag_answer"
      })
    );
    expect((await screen.findByTestId("mind-map-artifact")).textContent).toContain("产品定位 - 思维导图");
  });
});
