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

export interface RagAskCommand {
  question: string;
  limit?: number;
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
}
