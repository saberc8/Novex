export type DatasetQuery = {
  page?: number;
  size?: number;
  name?: string;
  status?: number;
};

export type DatasetResp = {
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
};

export type DatasetCommand = {
  name: string;
  description?: string;
  visibility?: number;
  retrievalMode?: number;
};

export type FileResp = {
  id: number;
  name?: string;
  originalName: string;
  size?: number;
  url?: string;
  parentPath?: string;
  path?: string;
  sha256?: string;
  contentType?: string;
  metadata?: string;
  createUserString?: string;
  createTime?: string;
  updateUserString?: string;
  updateTime?: string;
};

export type ParserJobResp = {
  id: number;
  tenantId?: number;
  datasetId?: number;
  documentId: number;
  jobType?: number;
  status: number;
  attemptCount?: number;
  errorMessage?: string;
  resultSummary?: Record<string, unknown>;
  documentName?: string;
  sourceUri?: string;
  fileId?: number | null;
  contentType?: string;
  parseStatus?: number;
  ingestionStatus?: number;
  chunkCount?: number;
  parserRequest?: Record<string, unknown> | null;
  createUserString?: string;
  createTime?: string;
  updateUserString?: string;
  updateTime?: string;
};

export type KnowledgeFileUploadResp = {
  file: FileResp;
  parseJob: ParserJobResp;
};
