import type { PageQuery } from "@/types/api";

export interface DatasetQuery extends PageQuery {
  name?: string;
  status?: number;
}

export interface DatasetCommand {
  name: string;
  description: string;
  visibility: number;
  retrievalMode: number;
}

export interface DatasetResp {
  id: number;
  tenantId: number;
  name: string;
  description: string;
  ownerId: number;
  visibility: number;
  status: number;
  retrievalMode: number;
  documentCount: number;
  chunkCount: number;
  createUserString: string;
  createTime: string;
  updateUserString: string;
  updateTime: string;
}

export interface DocumentQuery extends PageQuery {}

export interface DocumentResp {
  id: number;
  tenantId: number;
  datasetId: number;
  name: string;
  sourceUri: string;
  fileId?: number | null;
  contentType: string;
  ownerId: number;
  visibility: number;
  parseStatus: number;
  ingestionStatus: number;
  chunkCount: number;
  sourceHash: string;
  createUserString: string;
  createTime: string;
  updateUserString: string;
  updateTime: string;
}

export interface DocumentUploadCommand {
  name: string;
  content: string;
  contentType?: string;
}

export interface RagAskCommand {
  question: string;
  limit?: number;
  answerModelRouteId?: string;
}

export interface CitationResp {
  documentId: string;
  chunkId: string;
  pageNo?: number | null;
  sectionPath: string[];
}

export interface RagAskResp {
  traceId: number;
  answer: string;
  citations: CitationResp[];
  retrievalHitCount: number;
  answerStrategy: string;
  embeddingModelRoute: string;
  rerankModelRoute: string;
  answerModelRoute: string;
  answerModel?: string | null;
}

export interface StudioArtifactGenerateCommand {
  actionCode: string;
  topic: string;
  sessionId?: number | null;
  maxNodes?: number;
  answerModelRouteId?: string;
}

export interface MindMapNode {
  id: string;
  label: string;
  summary?: string;
  level?: number;
  citationRefs?: string[];
}

export interface MindMapEdge {
  source: string;
  target: string;
}

export interface MindMapCitation {
  id: string;
  documentId: string;
  chunkId: string;
  pageNo?: number | null;
  sectionPath: string[];
}

export interface MindMapContent {
  title: string;
  nodes: MindMapNode[];
  edges: MindMapEdge[];
  citations?: MindMapCitation[];
  metadata?: Record<string, unknown>;
}

export interface StudioArtifactResp {
  id: number;
  tenantId: number;
  datasetId?: number | null;
  sessionId?: number | null;
  runId?: number | null;
  ragTraceId?: number | null;
  actionCode: string;
  artifactType: string;
  title: string;
  contentJson: MindMapContent;
  contentText: string;
  sourceSnapshot: Record<string, unknown>;
  citations: CitationResp[];
  version: number;
  status: number;
  metadata: Record<string, unknown>;
  createUser: number;
  createTime: string;
  updateTime: string;
}
