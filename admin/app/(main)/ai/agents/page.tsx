"use client";

import {
  Check,
  CircleStop,
  Clock3,
  ListTree,
  Play,
  RefreshCw,
  RotateCw,
  TerminalSquare
} from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";
import {
  cancelAgentRun,
  createAgentRun,
  listAgentRunEvents,
  listAgentRuns,
  resumeAgentRun
} from "@/api/ai/agent";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import type { AgentRunEventResp, AgentRunResp, TaskBudget } from "@/types/ai-agent";

const DEFAULT_BUDGET: Required<TaskBudget> = {
  maxSteps: 6,
  maxToolCalls: 2,
  maxSeconds: 30,
  maxCostCents: 0
};

export default function AiAgentsPage() {
  const [runs, setRuns] = useState<AgentRunResp[]>([]);
  const [events, setEvents] = useState<AgentRunEventResp[]>([]);
  const [selectedRun, setSelectedRun] = useState<AgentRunResp | null>(null);
  const [runTotal, setRunTotal] = useState(0);
  const [input, setInput] = useState("send Feishu training reminder");
  const [autoApprove, setAutoApprove] = useState(false);
  const [budget, setBudget] = useState<Required<TaskBudget>>(DEFAULT_BUDGET);
  const [runsLoading, setRunsLoading] = useState(false);
  const [eventsLoading, setEventsLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [resuming, setResuming] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  const loadRuns = useCallback(async (preferredRunId?: number) => {
    setRunsLoading(true);
    try {
      const result = await listAgentRuns({ page: 1, size: 30 });
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
      toast.error(error instanceof Error ? error.message : "Agent Run 加载失败");
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
      toast.error(error instanceof Error ? error.message : "Event Snapshot 加载失败");
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

  async function submitRun(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmedInput = input.trim();
    if (!trimmedInput) {
      toast.error("请输入 Agent 输入");
      return;
    }
    setSubmitting(true);
    try {
      const run = await createAgentRun({
        input: trimmedInput,
        autoApprove,
        budget
      });
      setSelectedRun(run);
      await Promise.all([loadRuns(run.runId), loadEvents(run.runId)]);
      toast.success(`Run #${run.runId}`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Run 创建失败");
    } finally {
      setSubmitting(false);
    }
  }

  async function resumeSelectedRun() {
    if (!selectedRun) {
      return;
    }
    setResuming(true);
    try {
      const run = await resumeAgentRun(selectedRun.runId, {
        approved: true,
        input: { source: "admin" }
      });
      setSelectedRun(run);
      await Promise.all([loadRuns(run.runId), loadEvents(run.runId)]);
      toast.success(`Run #${run.runId} 已恢复`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Run 恢复失败");
    } finally {
      setResuming(false);
    }
  }

  async function cancelSelectedRun() {
    if (!selectedRun) {
      return;
    }
    setCancelling(true);
    try {
      const run = await cancelAgentRun(selectedRun.runId);
      setSelectedRun(run);
      await Promise.all([loadRuns(run.runId), loadEvents(run.runId)]);
      toast.success(`Run #${run.runId} 已取消`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Run 取消失败");
    } finally {
      setCancelling(false);
    }
  }

  const canResume = selectedRun?.status === "waiting_approval" || selectedRun?.status === "paused";
  const canCancel = selectedRun ? !["cancelled", "failed", "succeeded"].includes(selectedRun.status) : false;

  return (
    <div className="mx-auto grid w-full max-w-7xl items-start gap-4 xl:grid-cols-[380px_1fr]">
      <section className="rounded-lg border bg-background p-4">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div className="min-w-0">
            <h1 className="truncate text-base font-semibold">Agent Runtime</h1>
            <p className="text-xs text-muted-foreground">{runTotal} Runs</p>
          </div>
          <Button variant="outline" size="icon" title="刷新" onClick={() => void loadRuns()} disabled={runsLoading}>
            <RefreshCw />
          </Button>
        </div>

        <form className="grid gap-3" onSubmit={(event) => void submitRun(event)}>
          <div className="grid gap-1.5">
            <Label htmlFor="agent-input">输入</Label>
            <Textarea
              id="agent-input"
              value={input}
              className="min-h-28 resize-none"
              onChange={(event) => setInput(event.target.value)}
            />
          </div>

          <div className="grid grid-cols-2 gap-2">
            <BudgetInput
              label="Steps"
              value={budget.maxSteps}
              onChange={(value) => setBudget((current) => ({ ...current, maxSteps: value }))}
            />
            <BudgetInput
              label="Tools"
              value={budget.maxToolCalls}
              onChange={(value) => setBudget((current) => ({ ...current, maxToolCalls: value }))}
            />
            <BudgetInput
              label="Seconds"
              value={budget.maxSeconds}
              onChange={(value) => setBudget((current) => ({ ...current, maxSeconds: value }))}
            />
            <BudgetInput
              label="Cost"
              value={budget.maxCostCents}
              onChange={(value) => setBudget((current) => ({ ...current, maxCostCents: value }))}
            />
          </div>

          <div className="flex h-10 items-center justify-between rounded-md border px-3">
            <Label htmlFor="auto-approve" className="text-sm">Auto approve</Label>
            <Switch id="auto-approve" checked={autoApprove} onCheckedChange={setAutoApprove} />
          </div>

          <PermissionGate permissions={["ai:agent:run"]}>
            <Button type="submit" disabled={submitting}>
              <Play />
              Run
            </Button>
          </PermissionGate>
        </form>

        <div className="mt-4 grid gap-2">
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
              onClick={() => setSelectedRun(run)}
            >
              <div className="flex min-w-0 items-center justify-between gap-2">
                <span className="truncate text-sm font-medium">#{run.runId}</span>
                <Badge variant={statusVariant(run.status)}>{run.status}</Badge>
              </div>
              <div className="grid gap-1 text-xs text-muted-foreground">
                <span className="truncate">{run.intent}</span>
                <span className="truncate">{run.selectedToolCode || "-"}</span>
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
                {selectedRun ? `Run #${selectedRun.runId}` : "Run"}
              </h2>
              <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                {selectedRun ? (
                  <>
                    <Badge variant={statusVariant(selectedRun.status)}>{selectedRun.status}</Badge>
                    <span className="inline-flex items-center gap-1">
                      <TerminalSquare className="size-3.5" />
                      {selectedRun.intent}
                    </span>
                    <span className="inline-flex items-center gap-1">
                      <ListTree className="size-3.5" />
                      {selectedRun.traceId}
                    </span>
                  </>
                ) : (
                  <span>-</span>
                )}
              </div>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button variant="outline" onClick={() => void loadEvents()} disabled={!selectedRun || eventsLoading}>
                <RotateCw />
                Events
              </Button>
              <PermissionGate permissions={["ai:agent:resume"]}>
                <Button variant="outline" onClick={() => void resumeSelectedRun()} disabled={!canResume || resuming}>
                  <Check />
                  Resume
                </Button>
              </PermissionGate>
              <PermissionGate permissions={["ai:agent:cancel"]}>
                <Button variant="outline" onClick={() => void cancelSelectedRun()} disabled={!canCancel || cancelling}>
                  <CircleStop />
                  Cancel
                </Button>
              </PermissionGate>
            </div>
          </div>

          {selectedRun ? (
            <div className="mt-4 grid gap-3 md:grid-cols-3">
              <Metric label="Tool" value={selectedRun.selectedToolCode || "-"} />
              <Metric label="Pause" value={selectedRun.pauseReason || "-"} />
              <Metric label="Budget" value={`${selectedRun.taskBudget.maxSteps ?? "-"} / ${selectedRun.taskBudget.maxToolCalls ?? "-"}`} />
            </div>
          ) : null}

          {selectedRun?.finalOutput ? (
            <div className="mt-4 rounded-md border bg-muted/30 p-3 text-sm">{selectedRun.finalOutput}</div>
          ) : null}
        </div>

        <div className="rounded-lg border bg-background p-4">
          <div className="mb-3 flex items-center justify-between gap-3">
            <h2 className="text-sm font-medium">Event Snapshot</h2>
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
                <pre className="max-h-36 overflow-auto rounded-md bg-muted p-2 text-xs leading-relaxed text-muted-foreground">
                  {payloadPreview(event.payload)}
                </pre>
              </div>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}

function BudgetInput({
  label,
  value,
  onChange
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="grid gap-1.5">
      <Label className="text-xs">{label}</Label>
      <Input
        type="number"
        min={0}
        value={value}
        onChange={(event) => onChange(numberValue(event.target.value, value))}
      />
    </div>
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

function numberValue(value: string, fallback: number) {
  if (value === "") {
    return 0;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
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
