import type { AgentRunEventResp } from "@/types/agent";

export interface ModelDeltaSummary {
  text: string;
  chunkCount: number;
  routeId?: string;
  provider?: string;
  model?: string;
}

interface ModelDeltaChunk {
  content: string;
  sequenceNo: number;
  deltaIndex?: number;
  routeId?: string;
  provider?: string;
  model?: string;
}

export function summarizeModelDeltas(events: AgentRunEventResp[]): ModelDeltaSummary | null {
  const chunks = events
    .map(modelDeltaChunkFromEvent)
    .filter((chunk): chunk is ModelDeltaChunk => chunk !== null)
    .sort((left, right) => {
      const leftIndex = left.deltaIndex ?? Number.MAX_SAFE_INTEGER;
      const rightIndex = right.deltaIndex ?? Number.MAX_SAFE_INTEGER;
      return leftIndex - rightIndex || left.sequenceNo - right.sequenceNo;
    });

  if (chunks.length === 0) {
    return null;
  }

  const firstWithMetadata =
    chunks.find((chunk) => chunk.routeId || chunk.provider || chunk.model) ?? chunks[0];

  return {
    text: chunks.map((chunk) => chunk.content).join(""),
    chunkCount: chunks.length,
    routeId: firstWithMetadata.routeId,
    provider: firstWithMetadata.provider,
    model: firstWithMetadata.model
  };
}

function modelDeltaChunkFromEvent(event: AgentRunEventResp): ModelDeltaChunk | null {
  const item = eventPayloadItem(event.payload);
  if (!item || stringValue(item.type) !== "model_delta") {
    return null;
  }
  const content = typeof item.content === "string" ? item.content : null;
  if (content === null || content.length === 0) {
    return null;
  }

  return {
    content,
    sequenceNo: event.sequenceNo,
    deltaIndex: numberValue(item.deltaIndex),
    routeId: stringValue(item.routeId),
    provider: stringValue(item.provider),
    model: stringValue(item.model)
  };
}

function eventPayloadItem(payload: AgentRunEventResp["payload"]): Record<string, unknown> | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    return null;
  }
  const payloadRecord = payload as Record<string, unknown>;
  const item = payloadRecord.item;
  if (item && typeof item === "object" && !Array.isArray(item)) {
    return item as Record<string, unknown>;
  }
  return payloadRecord;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}
