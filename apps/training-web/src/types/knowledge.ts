import type { PageQuery } from "./api";

export interface DatasetQuery extends PageQuery {
  name?: string;
  status?: number;
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

export interface FileResp {
  id: number;
  name: string;
  originalName: string;
  size: number;
  url: string;
  parentPath: string;
  path: string;
  sha256: string;
  contentType: string;
  metadata: string;
  thumbnailSize: number;
  thumbnailName: string;
  thumbnailMetadata: string;
  thumbnailUrl: string;
  extension: string;
  type: number;
  storageId: number;
  storageName: string;
  createUserString: string;
  createTime: string;
  updateUserString: string;
  updateTime: string;
}

export interface ParserJobResp {
  id: number;
  tenantId: number;
  datasetId: number;
  documentId: number;
  jobType: number;
  status: number;
  attemptCount: number;
  errorMessage: string;
  resultSummary: Record<string, unknown>;
  documentName: string;
  sourceUri: string;
  fileId?: number | null;
  contentType: string;
  parseStatus: number;
  ingestionStatus: number;
  chunkCount: number;
  parserRequest?: Record<string, unknown> | null;
  createUserString: string;
  createTime: string;
  updateUserString: string;
  updateTime: string;
}

export interface KnowledgeFileUploadResp {
  file: FileResp;
  parseJob: ParserJobResp;
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

export type RagFeedbackRating = "helpful" | "not_helpful" | "citation_issue";

export interface RagFeedbackCommand {
  traceId: number;
  rating: RagFeedbackRating;
  reason?: string;
}

export interface RagFeedbackResp {
  id: number;
  traceId: number;
  rating: RagFeedbackRating;
}

export interface AiFeedbackCommand {
  resourceType: string;
  resourceId: string;
  traceId?: string;
  rating: string;
  reason?: string;
  metadata?: Record<string, unknown>;
}

export interface AiFeedbackResp {
  id: number;
  resourceType: string;
  resourceId: string;
  traceId?: string | null;
  rating: string;
}
