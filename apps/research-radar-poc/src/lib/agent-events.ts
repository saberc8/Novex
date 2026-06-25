import type { AgentRunEventResp } from "@/types/agent";

export type ModelDeltaSummary = {
  text: string;
  chunkCount: number;
  routeId?: string;
  model?: string;
};

export type ResearchEventEvidence = {
  sequenceNo: number;
  title: string;
  kind: string;
  text: string;
};

type ModelDeltaChunk = {
  content: string;
  sequenceNo: number;
  deltaIndex?: number;
  routeId?: string;
  model?: string;
};

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

  const firstWithMetadata = chunks.find((chunk) => chunk.routeId || chunk.model) ?? chunks[0];

  return {
    text: chunks.map((chunk) => chunk.content).join(""),
    chunkCount: chunks.length,
    routeId: firstWithMetadata.routeId,
    model: firstWithMetadata.model
  };
}

export function summarizeResearchEvent(event: AgentRunEventResp): ResearchEventEvidence {
  const item = eventPayloadItem(event.payload);
  const itemType = stringValue(item?.type);

  if (itemType === "model_delta") {
    return {
      sequenceNo: event.sequenceNo,
      title: "Assistant",
      kind: "model",
      text: stringValue(item?.content) ?? "model output chunk"
    };
  }

  if (itemType === "tool_observation") {
    const toolCode = stringValue(item?.toolCode) ?? "tool";
    const output = objectValue(item?.output);
    return {
      sequenceNo: event.sequenceNo,
      title: titleForTool(toolCode),
      kind: "tool",
      text: toolObservationText(toolCode, output)
    };
  }

  return {
    sequenceNo: event.sequenceNo,
    title: event.eventType,
    kind: event.status,
    text: eventText(event)
  };
}

function modelDeltaChunkFromEvent(event: AgentRunEventResp): ModelDeltaChunk | null {
  const item = eventPayloadItem(event.payload);
  if (!item || stringValue(item.type) !== "model_delta") {
    return null;
  }
  const content = stringValue(item.content);
  if (!content) {
    return null;
  }

  return {
    content,
    sequenceNo: event.sequenceNo,
    deltaIndex: numberValue(item.deltaIndex),
    routeId: stringValue(item.routeId),
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

function toolObservationText(toolCode: string, output: Record<string, unknown> | null) {
  if (output?.dryRun === true || output?.status === "dry_run") {
    return `dry-run: ${toolCode} returned no live provider result`;
  }
  if (typeof output?.status === "string") {
    return `${output.status}: ${toolCode}`;
  }
  return `${toolCode} returned an observation`;
}

function titleForTool(toolCode: string) {
  const title = toolCode
    .split(".")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
  return title.charAt(0) + title.slice(1).toLowerCase();
}

function eventText(event: AgentRunEventResp) {
  if (typeof event.payload === "string") {
    return event.payload;
  }
  return `${event.eventType} ${event.status}`.trim();
}

function objectValue(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}
