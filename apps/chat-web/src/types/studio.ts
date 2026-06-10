import type { CitationResp } from "./knowledge";

export interface StudioActionQuery {
  surface?: string;
}

export interface StudioActionResp {
  id: number;
  tenantId: number;
  code: string;
  name: string;
  description: string;
  surface: string;
  artifactType: string;
  pluginCode?: string | null;
  skillCode?: string | null;
  permissionCode: string;
  modelRoutePolicy: Record<string, unknown>;
  inputSchema: Record<string, unknown>;
  outputSchema: Record<string, unknown>;
  renderer: string;
  sort: number;
  status: number;
  metadata: Record<string, unknown>;
  createTime: string;
}

export interface StudioArtifactGenerateCommand {
  actionCode: string;
  topic?: string;
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
  sectionPath?: string[];
}

export interface MindMapContent {
  title: string;
  nodes: MindMapNode[];
  edges: MindMapEdge[];
  citations: MindMapCitation[];
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
  contentJson: Record<string, unknown>;
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
