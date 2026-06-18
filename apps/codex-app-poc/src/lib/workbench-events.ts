import type { AgentRunEventResp } from "@/types/agent";

export type WorkbenchEventKind =
  | "assistant_delta"
  | "model"
  | "tool"
  | "retrieval"
  | "mcp"
  | "web_search"
  | "terminal"
  | "error"
  | "raw";

export type WorkbenchEventEvidence = {
  kind: WorkbenchEventKind;
  title: string;
  text: string;
  status: string;
  sequenceNo: number;
  raw: AgentRunEventResp["payload"];
};

export function summarizeWorkbenchEvent(event: AgentRunEventResp): WorkbenchEventEvidence {
  const payload = objectPayload(event.payload) ?? {};
  const item = objectPayload(payload.item);
  const type = stringValue(item?.type);
  const toolCode = stringValue(payload.toolCode) || stringValue(item?.toolCode);
  const output = objectPayload(item?.output) ?? objectPayload(payload.output);
  const args = payload.arguments ?? item?.arguments;

  if (type === "model_delta") {
    return evidence(event, "assistant_delta", "Assistant", stringValue(item?.content) ?? "");
  }

  if (toolCode === "rag.search" && output) {
    const hits = Array.isArray(output.hits) ? output.hits.length : 0;
    return evidence(event, "retrieval", "Knowledge search", `${hits} hits from rag.search`);
  }

  if (toolCode === "web.search") {
    const dryRun = output?.dryRun === true || output?.status === "dry_run";
    return evidence(
      event,
      "web_search",
      "Web search",
      dryRun ? "web.search dry-run; provider is not configured" : "web.search returned results"
    );
  }

  if (toolCode?.startsWith("mcp.")) {
    return evidence(event, "mcp", toolCode, compactJson(args ?? output ?? payload));
  }

  if (toolCode) {
    return evidence(event, "tool", toolCode, compactJson(args ?? output ?? payload));
  }

  if (event.status === "failed" || event.eventType === "error") {
    return evidence(event, "error", "Error", stringValue(payload.message) ?? "Agent run failed");
  }

  if (["succeeded", "cancelled", "waiting_approval"].includes(event.status)) {
    return evidence(event, "terminal", event.status, stringValue(payload.message) ?? event.status);
  }

  return evidence(event, "raw", event.eventType, compactJson(event.payload));
}

function evidence(
  event: AgentRunEventResp,
  kind: WorkbenchEventKind,
  title: string,
  text: string
): WorkbenchEventEvidence {
  return {
    kind,
    title,
    text,
    status: event.status,
    sequenceNo: event.sequenceNo,
    raw: event.payload
  };
}

function objectPayload(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function compactJson(value: unknown): string {
  return JSON.stringify(value ?? {});
}
