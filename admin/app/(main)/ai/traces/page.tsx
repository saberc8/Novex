"use client";

import { Clock3, History, ListTree, RefreshCw, TerminalSquare } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { listAgentRunEvents, listAgentRuns } from "@/api/ai/agent";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { AgentRunEventResp, AgentRunResp } from "@/types/ai-agent";

export default function AiTracesPage() {
  const [runs, setRuns] = useState<AgentRunResp[]>([]);
  const [events, setEvents] = useState<AgentRunEventResp[]>([]);
  const [selectedRun, setSelectedRun] = useState<AgentRunResp | null>(null);
  const [runTotal, setRunTotal] = useState(0);
  const [runsLoading, setRunsLoading] = useState(false);
  const [eventsLoading, setEventsLoading] = useState(false);

  const loadRuns = useCallback(async (preferredRunId?: number) => {
    setRunsLoading(true);
    try {
      const result = await listAgentRuns({ page: 1, size: 20 });
      setRuns(result.list);
      setRunTotal(result.total);
      setSelectedRun((current) => {
        if (!result.list.length) {
          return null;
        }
        if (preferredRunId) {
          return result.list.find((run) => run.runId === preferredRunId) ?? result.list[0];
        }
        if (!current) {
          return result.list[0];
        }
        return result.list.find((run) => run.runId === current.runId) ?? result.list[0];
      });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Trace Run 加载失败");
    } finally {
      setRunsLoading(false);
    }
  }, []);

  const loadEvents = useCallback(async (runId?: number) => {
    const targetRunId = runId ?? selectedRun?.runId;
    if (!targetRunId) {
      setEvents([]);
      return;
    }
    setEventsLoading(true);
    try {
      const result = await listAgentRunEvents(targetRunId, { page: 1, size: 100 });
      setEvents(result.list);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Replay Snapshot 加载失败");
    } finally {
      setEventsLoading(false);
    }
  }, [selectedRun?.runId]);

  useEffect(() => {
    void loadRuns();
  }, [loadRuns]);

  useEffect(() => {
    void loadEvents();
  }, [loadEvents]);

  function selectRun(run: AgentRunResp) {
    setSelectedRun(run);
    void loadEvents(run.runId);
  }

  return (
    <PermissionGate permissions={["ai:trace:list"]}>
      <div className="mx-auto grid w-full max-w-7xl items-start gap-4 xl:grid-cols-[360px_1fr]">
        <section className="rounded-lg border bg-background p-4">
          <div className="mb-3 flex items-center justify-between gap-3">
            <div className="min-w-0">
              <div className="inline-flex items-center gap-2 rounded-md border bg-muted/40 px-2 py-1 text-xs text-muted-foreground">
                <History className="size-3.5 text-primary" />
                Run Trace
              </div>
              <h1 className="mt-2 truncate text-base font-semibold">运行追踪</h1>
              <p className="mt-1 text-xs text-muted-foreground">{runTotal} Runs</p>
            </div>
            <Button variant="outline" size="icon" title="刷新" onClick={() => void loadRuns()} disabled={runsLoading}>
              <RefreshCw />
            </Button>
          </div>

          <div className="grid gap-2">
            {runsLoading ? (
              <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">加载中</div>
            ) : null}
            {!runsLoading && !runs.length ? (
              <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无 Run</div>
            ) : null}
            {runs.map((run) => (
              <button
                key={run.runId}
                type="button"
                className={[
                  "grid w-full gap-2 rounded-md border p-3 text-left transition-colors hover:bg-muted/40",
                  selectedRun?.runId === run.runId ? "border-primary bg-muted/45" : "bg-background"
                ].join(" ")}
                onClick={() => selectRun(run)}
              >
                <div className="flex min-w-0 items-center justify-between gap-2">
                  <span className="truncate text-sm font-medium">#{run.runId}</span>
                  <Badge variant={statusVariant(run.status)}>{run.status}</Badge>
                </div>
                <div className="grid gap-1 text-xs text-muted-foreground">
                  <span className="truncate">{run.traceId}</span>
                  <span className="truncate">{run.intent}</span>
                </div>
              </button>
            ))}
          </div>
        </section>

        <section className="grid gap-4">
          <div className="rounded-lg border bg-background p-4">
            <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
              <div className="min-w-0">
                <h2 className="truncate text-base font-semibold">
                  {selectedRun ? `Run #${selectedRun.runId}` : "Run Trace"}
                </h2>
                <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                  {selectedRun ? (
                    <>
                      <Badge variant={statusVariant(selectedRun.status)}>{selectedRun.status}</Badge>
                      <span className="inline-flex items-center gap-1">
                        <TerminalSquare className="size-3.5" />
                        {selectedRun.traceId}
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <ListTree className="size-3.5" />
                        {selectedRun.loopKind}
                      </span>
                    </>
                  ) : (
                    <span>-</span>
                  )}
                </div>
              </div>
              <Button variant="outline" onClick={() => void loadEvents()} disabled={!selectedRun || eventsLoading}>
                <RefreshCw />
                Events
              </Button>
            </div>

            {selectedRun ? (
              <div className="mt-4 grid gap-3 md:grid-cols-4">
                <Metric label="Intent" value={selectedRun.intent} />
                <Metric label="Tool" value={selectedRun.selectedToolCode || "-"} />
                <Metric label="Pause" value={selectedRun.pauseReason || "-"} />
                <Metric label="Budget" value={`${selectedRun.taskBudget.maxSteps ?? "-"} steps`} />
              </div>
            ) : null}

            {selectedRun?.finalOutput ? (
              <div className="mt-4 rounded-md border bg-muted/30 p-3 text-sm">{selectedRun.finalOutput}</div>
            ) : null}
          </div>

          <div className="rounded-lg border bg-background p-4">
            <div className="mb-3 flex items-center justify-between gap-3">
              <h2 className="text-sm font-medium">Event Replay Snapshot</h2>
              <div className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                <Clock3 className="size-3.5" />
                {events.length}
              </div>
            </div>

            <div className="grid gap-2">
              {eventsLoading ? (
                <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">加载中</div>
              ) : null}
              {!eventsLoading && !events.length ? (
                <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无 Event</div>
              ) : null}
              {events.map((event) => (
                <div key={event.id} className="grid gap-2 rounded-md border p-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-2">
                      <Badge variant="outline">#{event.sequenceNo}</Badge>
                      <span className="truncate text-sm font-medium">{event.eventType}</span>
                    </div>
                    <Badge variant={statusVariant(event.status)}>{event.status}</Badge>
                  </div>
                  <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-muted p-2 text-xs leading-relaxed text-muted-foreground">
                    {payloadPreview(event.payload)}
                  </pre>
                </div>
              ))}
            </div>
          </div>
        </section>
      </div>
    </PermissionGate>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md border bg-muted/20 p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 truncate text-sm font-medium">{value}</div>
    </div>
  );
}

function payloadPreview(payload: AgentRunEventResp["payload"]) {
  const text = typeof payload === "string" ? payload : JSON.stringify(payload, null, 2);
  if (!text) {
    return "-";
  }
  return text.length > 900 ? `${text.slice(0, 900)}...` : text;
}

function statusVariant(status: string) {
  if (status === "succeeded") {
    return "secondary";
  }
  if (status === "failed" || status === "cancelled") {
    return "destructive";
  }
  return "outline";
}
