"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Brain,
  CheckCircle2,
  Clock3,
  DatabaseZap,
  GitBranch,
  History,
  ListChecks,
  PauseCircle,
  Play,
  RotateCw,
  Send,
  ShieldCheck,
  Square,
  Wrench,
  XCircle
} from "lucide-react";
import {
  cancelAgentRun,
  createAgentRun,
  listAgentRunEvents,
  listAgentRuns,
  resumeAgentRun
} from "@/api/agent";
import type { AgentRunEventResp, AgentRunResp, TaskBudget } from "@/types/agent";

const defaultBudget: Required<TaskBudget> = {
  maxSteps: 6,
  maxToolCalls: 1,
  maxSeconds: 30,
  maxCostCents: 0
};

const fallbackRun: AgentRunResp = {
  runId: 0,
  traceId: "standby",
  status: "idle",
  intent: "tool_task",
  loopKind: "react",
  selectedToolCode: "rag.search",
  pauseReason: null,
  finalOutput: "Start an agent run to inspect the workflow, tool calls, approvals, and result.",
  taskBudget: defaultBudget,
  createTime: "2026-06-05 10:00:00",
  updateTime: null
};

const fallbackEvents: AgentRunEventResp[] = [
  {
    id: 0,
    runId: 0,
    stepId: null,
    eventType: "workspace_ready",
    sequenceNo: 1,
    status: "idle",
    payload: {
      workflow: "react",
      tools: ["rag.search", "feishu.message.send", "media.image.generate"]
    },
    createTime: "2026-06-05 10:00:00"
  }
];

const navItems = [
  { label: "Runs", icon: History },
  { label: "Workflow", icon: GitBranch },
  { label: "Tools", icon: Wrench },
  { label: "Memory", icon: Brain }
];

const availableTools = [
  { code: "rag.search", risk: "low", permission: "ai:knowledge:ask" },
  { code: "feishu.message.send", risk: "medium", permission: "ai:agent:resume" },
  { code: "media.image.generate", risk: "medium", permission: "ai:tool:dryRun" }
];

const memoryRows = [
  { label: "Session", value: "current task context" },
  { label: "Project", value: "tool approvals and recent runs" },
  { label: "Policy", value: "RBAC, budget, audit trace" }
];

export function AgentWorkspaceClient() {
  const [runs, setRuns] = useState<AgentRunResp[]>([fallbackRun]);
  const [events, setEvents] = useState<AgentRunEventResp[]>(fallbackEvents);
  const [selectedRun, setSelectedRun] = useState<AgentRunResp>(fallbackRun);
  const [taskInput, setTaskInput] = useState("send Feishu training reminder");
  const [autoApprove, setAutoApprove] = useState(false);
  const [budget, setBudget] = useState<Required<TaskBudget>>(defaultBudget);
  const [apiStatus, setApiStatus] = useState("fallback");
  const [loadingRuns, setLoadingRuns] = useState(false);
  const [loadingEvents, setLoadingEvents] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [resuming, setResuming] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [notice, setNotice] = useState("");
  const selectedRunIdRef = useRef(fallbackRun.runId);

  const selectRun = useCallback((run: AgentRunResp) => {
    selectedRunIdRef.current = run.runId;
    setSelectedRun(run);
  }, []);

  const loadEvents = useCallback(async (runId: number) => {
    if (runId <= 0) {
      setEvents(fallbackEvents);
      return;
    }
    setLoadingEvents(true);
    try {
      const page = await listAgentRunEvents(runId, { page: 1, size: 100 });
      setEvents(page.list.length > 0 ? page.list : []);
      setApiStatus("live");
    } catch {
      setApiStatus("fallback");
    } finally {
      setLoadingEvents(false);
    }
  }, []);

  const loadRuns = useCallback(
    async (preferredRunId?: number) => {
      setLoadingRuns(true);
      try {
        const page = await listAgentRuns({ page: 1, size: 20 });
        const nextRuns = page.list.length > 0 ? page.list : [fallbackRun];
        const nextSelected =
          nextRuns.find((run) => run.runId === preferredRunId) ??
          nextRuns.find((run) => run.runId === selectedRunIdRef.current) ??
          nextRuns[0];
        setRuns(nextRuns);
        selectRun(nextSelected);
        setApiStatus("live");
        await loadEvents(nextSelected.runId);
      } catch {
        setApiStatus("fallback");
      } finally {
        setLoadingRuns(false);
      }
    },
    [loadEvents, selectRun]
  );

  useEffect(() => {
    void loadRuns();
  }, [loadRuns]);

  const canApprove = selectedRun.status === "waiting_approval" || selectedRun.status === "paused";
  const canCancel = selectedRun.runId > 0 && !["cancelled", "failed", "succeeded"].includes(selectedRun.status);
  const statusTone = statusClass(selectedRun.status);

  async function handleStartRun() {
    const input = taskInput.trim();
    if (!input || submitting) {
      return;
    }
    setSubmitting(true);
    setNotice("");
    try {
      const run = await createAgentRun({
        input,
        autoApprove,
        budget
      });
      selectRun(run);
      setRuns((current) => upsertRun(current, run));
      await loadEvents(run.runId);
      setApiStatus("live");
    } catch {
      setNotice("Run creation failed. Check token, permission, or backend availability.");
      setApiStatus("fallback");
    } finally {
      setSubmitting(false);
    }
  }

  async function handleApproveRun() {
    if (!canApprove || resuming) {
      return;
    }
    setResuming(true);
    setNotice("");
    try {
      const run = await resumeAgentRun(selectedRun.runId, {
        approved: true,
        input: { source: "agent-workspace" }
      });
      selectRun(run);
      setRuns((current) => upsertRun(current, run));
      await loadEvents(run.runId);
      setApiStatus("live");
    } catch {
      setNotice("Resume failed. Approval permission may be missing.");
      setApiStatus("fallback");
    } finally {
      setResuming(false);
    }
  }

  async function handleCancelRun() {
    if (!canCancel || cancelling) {
      return;
    }
    setCancelling(true);
    setNotice("");
    try {
      const run = await cancelAgentRun(selectedRun.runId);
      selectRun(run);
      setRuns((current) => upsertRun(current, run));
      await loadEvents(run.runId);
      setApiStatus("live");
    } catch {
      setNotice("Cancel failed. The run may already be terminal.");
      setApiStatus("fallback");
    } finally {
      setCancelling(false);
    }
  }

  const variables = useMemo(
    () => [
      { label: "Intent", value: selectedRun.intent },
      { label: "Loop", value: selectedRun.loopKind },
      { label: "Trace", value: selectedRun.traceId },
      { label: "Tool", value: selectedRun.selectedToolCode || "-" }
    ],
    [selectedRun]
  );

  return (
    <main className="min-h-screen bg-slate-100 text-slate-950">
      <div className="mx-auto grid min-h-screen max-w-[1480px] grid-cols-1 lg:grid-cols-[260px_minmax(0,1fr)_360px]">
        <aside className="border-b border-slate-200 bg-white p-4 lg:border-b-0 lg:border-r lg:p-5">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-teal-700 text-sm font-semibold text-white">
              AG
            </div>
            <div className="min-w-0">
              <div className="text-sm font-semibold text-slate-950">Novex</div>
              <div className="truncate text-xs text-slate-500">Agent Workspace</div>
            </div>
          </div>

          <nav aria-label="Agent navigation" className="mt-5 space-y-2">
            {navItems.map((item, index) => (
              <button
                className={[
                  "flex h-10 w-full items-center gap-3 rounded-lg border px-3 text-left text-sm font-medium",
                  index === 0
                    ? "border-teal-200 bg-teal-50 text-teal-950"
                    : "border-transparent text-slate-600 hover:border-slate-200 hover:bg-slate-50"
                ].join(" ")}
                key={item.label}
                type="button"
              >
                <item.icon aria-hidden="true" className="h-4 w-4 shrink-0" />
                {item.label}
              </button>
            ))}
          </nav>

          <section className="mt-5 rounded-lg border border-slate-200 p-3">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
              <ShieldCheck aria-hidden="true" className="h-4 w-4 text-teal-700" />
              Execution policy
            </div>
            <div className="mt-2 text-xs leading-5 text-slate-500">
              Runs follow RBAC, tool risk, task budget, approval pause, trace, and audit controls.
            </div>
          </section>

          <section className="mt-3 rounded-lg border border-slate-200 p-3">
            <div className="text-xs font-semibold uppercase tracking-wide text-slate-500">Runtime</div>
            <div className="mt-2 inline-flex rounded-md bg-slate-100 px-2 py-1 text-xs font-medium text-slate-700">
              {apiStatus === "live" ? "Live backend" : "Fallback view"}
            </div>
          </section>

          <section className="mt-3 rounded-lg border border-slate-200 p-3">
            <div className="flex items-center justify-between gap-2">
              <div className="text-xs font-semibold uppercase tracking-wide text-slate-500">Recent runs</div>
              {loadingRuns ? <span className="text-xs text-slate-400">sync</span> : null}
            </div>
            <div className="mt-2 space-y-2">
              {runs.slice(0, 4).map((run) => (
                <button
                  className={[
                    "w-full rounded-md border p-2 text-left",
                    run.runId === selectedRun.runId
                      ? "border-teal-200 bg-teal-50"
                      : "border-slate-200 hover:bg-slate-50"
                  ].join(" ")}
                  key={run.runId}
                  onClick={() => {
                    selectRun(run);
                    void loadEvents(run.runId);
                  }}
                  type="button"
                >
                  <div className="flex min-w-0 items-center justify-between gap-2">
                    <span className="truncate text-xs font-semibold text-slate-900">Run #{run.runId}</span>
                    <span className={`shrink-0 rounded px-1.5 py-0.5 text-[11px] font-medium ${statusClass(run.status)}`}>
                      {run.status}
                    </span>
                  </div>
                  <div className="mt-1 truncate text-xs text-slate-500">
                    {run.selectedToolCode ? `Uses ${run.selectedToolCode}` : run.intent}
                  </div>
                </button>
              ))}
            </div>
          </section>
        </aside>

        <section className="min-w-0 p-4 lg:p-6">
          <header className="flex flex-col gap-3 border-b border-slate-200 pb-4 md:flex-row md:items-start md:justify-between">
            <div className="min-w-0">
              <div className="text-sm font-medium text-teal-700">Agent Runtime</div>
              <h1 className="mt-2 text-2xl font-semibold tracking-normal text-slate-950">Novex Agent</h1>
              <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-600">
                Run bounded tool tasks, inspect the workflow, and approve external actions before they execute.
              </p>
            </div>
            <span className={`inline-flex w-fit items-center gap-2 rounded-md px-3 py-2 text-sm font-medium ${statusTone}`}>
              <StatusIcon status={selectedRun.status} />
              {selectedRun.status}
            </span>
          </header>

          <div className="mt-5 grid gap-4 xl:grid-cols-[minmax(0,1fr)_300px]">
            <section className="rounded-lg border border-slate-200 bg-white shadow-sm">
              <div className="border-b border-slate-200 p-4">
                <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                  <div className="min-w-0">
                    <h2 className="text-sm font-semibold text-slate-950">Task</h2>
                    <div className="mt-1 text-xs text-slate-500">Create a controlled ReAct run graph.</div>
                  </div>
                  <label className="inline-flex w-fit items-center gap-2 rounded-md border border-slate-200 px-3 py-2 text-xs font-medium text-slate-600">
                    <input
                      checked={autoApprove}
                      className="h-4 w-4 accent-teal-700"
                      onChange={(event) => setAutoApprove(event.target.checked)}
                      type="checkbox"
                    />
                    Auto approve low risk
                  </label>
                </div>
              </div>
              <div className="p-4">
                <label className="text-xs font-semibold uppercase tracking-wide text-slate-500" htmlFor="agent-task">
                  Describe the task
                </label>
                <textarea
                  aria-label="Describe the task"
                  className="mt-2 min-h-28 w-full resize-none rounded-lg border border-slate-200 px-3 py-2 text-sm leading-6 outline-none focus:border-teal-500"
                  id="agent-task"
                  onChange={(event) => setTaskInput(event.target.value)}
                  value={taskInput}
                />
                <div className="mt-3 grid gap-2 sm:grid-cols-4">
                  <BudgetField label="Steps" value={budget.maxSteps} onChange={(value) => setBudget((current) => ({ ...current, maxSteps: value }))} />
                  <BudgetField label="Tools" value={budget.maxToolCalls} onChange={(value) => setBudget((current) => ({ ...current, maxToolCalls: value }))} />
                  <BudgetField label="Seconds" value={budget.maxSeconds} onChange={(value) => setBudget((current) => ({ ...current, maxSeconds: value }))} />
                  <BudgetField label="Cost" value={budget.maxCostCents} onChange={(value) => setBudget((current) => ({ ...current, maxCostCents: value }))} />
                </div>
                <div className="mt-4 flex flex-wrap items-center gap-2">
                  <button
                    className="inline-flex h-10 items-center gap-2 rounded-lg bg-teal-700 px-4 text-sm font-semibold text-white hover:bg-teal-800 disabled:bg-slate-300"
                    disabled={submitting}
                    onClick={() => void handleStartRun()}
                    type="button"
                  >
                    <Send aria-hidden="true" className="h-4 w-4" />
                    Start run
                  </button>
                  <button
                    className="inline-flex h-10 items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 text-sm font-semibold text-slate-700 hover:bg-slate-50 disabled:text-slate-300"
                    disabled={loadingRuns}
                    onClick={() => void loadRuns()}
                    type="button"
                  >
                    <RotateCw aria-hidden="true" className="h-4 w-4" />
                    Refresh
                  </button>
                  {notice ? <span className="text-sm text-rose-700">{notice}</span> : null}
                </div>
              </div>
            </section>

            <section className="rounded-lg border border-slate-200 bg-white p-4 shadow-sm">
              <div className="flex items-center gap-2">
                <ListChecks aria-hidden="true" className="h-4 w-4 text-teal-700" />
                <h2 className="text-sm font-semibold text-slate-950">Run summary</h2>
              </div>
              <div className="mt-3 space-y-2">
                {variables.map((item) => (
                  <Metric key={item.label} label={item.label} value={item.value} />
                ))}
              </div>
            </section>
          </div>

          <section className="mt-4 rounded-lg border border-slate-200 bg-white shadow-sm">
            <div className="flex flex-col gap-3 border-b border-slate-200 p-4 md:flex-row md:items-center md:justify-between">
              <div className="min-w-0">
                <h2 className="text-sm font-semibold text-slate-950">Workflow</h2>
                <div className="mt-1 text-xs text-slate-500">Run graph event snapshot and tool observations.</div>
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  className="inline-flex h-9 items-center gap-2 rounded-md border border-teal-200 bg-teal-50 px-3 text-sm font-semibold text-teal-900 hover:bg-teal-100 disabled:border-slate-200 disabled:bg-slate-50 disabled:text-slate-300"
                  disabled={!canApprove || resuming}
                  onClick={() => void handleApproveRun()}
                  type="button"
                >
                  <CheckCircle2 aria-hidden="true" className="h-4 w-4" />
                  Approve run
                </button>
                <button
                  className="inline-flex h-9 items-center gap-2 rounded-md border border-slate-200 bg-white px-3 text-sm font-semibold text-slate-700 hover:bg-slate-50 disabled:text-slate-300"
                  disabled={!canCancel || cancelling}
                  onClick={() => void handleCancelRun()}
                  type="button"
                >
                  <Square aria-hidden="true" className="h-4 w-4" />
                  Cancel run
                </button>
              </div>
            </div>
            <div className="grid gap-3 p-4">
              {loadingEvents ? (
                <div className="rounded-md border border-dashed border-slate-300 p-6 text-center text-sm text-slate-500">
                  Loading events
                </div>
              ) : null}
              {!loadingEvents && events.length === 0 ? (
                <div className="rounded-md border border-dashed border-slate-300 p-6 text-center text-sm text-slate-500">
                  No events
                </div>
              ) : null}
              {events.map((event) => (
                <article className="rounded-lg border border-slate-200 p-3" key={event.id}>
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-2">
                      <span className="inline-flex h-6 items-center rounded-md bg-slate-100 px-2 text-xs font-semibold text-slate-600">
                        #{event.sequenceNo}
                      </span>
                      <span className="truncate text-sm font-semibold text-slate-900">{event.eventType}</span>
                    </div>
                    <span className={`inline-flex rounded-md px-2 py-1 text-xs font-medium ${statusClass(event.status)}`}>
                      {event.status}
                    </span>
                  </div>
                  <pre className="mt-3 max-h-40 overflow-auto rounded-md bg-slate-50 p-3 text-xs leading-5 text-slate-600">
                    {payloadPreview(event.payload)}
                  </pre>
                </article>
              ))}
            </div>
          </section>
        </section>

        <aside className="space-y-4 border-t border-slate-200 bg-white p-4 lg:border-l lg:border-t-0 lg:p-5">
          <section className="rounded-lg border border-slate-200 p-4">
            <div className="flex items-center gap-2">
              <PauseCircle aria-hidden="true" className="h-4 w-4 text-teal-700" />
              <h2 className="text-sm font-semibold text-slate-950">Approval</h2>
            </div>
            <div className="mt-3 rounded-md bg-slate-50 p-3 text-sm leading-6 text-slate-700">
              {selectedRun.pauseReason ? `Pause reason: ${selectedRun.pauseReason}` : "No approval is currently required."}
            </div>
          </section>

          <section className="rounded-lg border border-slate-200 p-4">
            <div className="flex items-center gap-2">
              <Wrench aria-hidden="true" className="h-4 w-4 text-teal-700" />
              <h2 className="text-sm font-semibold text-slate-950">Tools</h2>
            </div>
            <div className="mt-3 space-y-2">
              {availableTools.map((tool) => (
                <div className="rounded-md border border-slate-200 p-3" key={tool.code}>
                  <div className="truncate text-sm font-semibold text-slate-900">Tool {tool.code}</div>
                  <div className="mt-1 flex flex-wrap gap-2 text-xs text-slate-500">
                    <span>{tool.risk} risk</span>
                    <span>{tool.permission}</span>
                  </div>
                </div>
              ))}
            </div>
          </section>

          <section className="rounded-lg border border-slate-200 p-4">
            <div className="flex items-center gap-2">
              <DatabaseZap aria-hidden="true" className="h-4 w-4 text-teal-700" />
              <h2 className="text-sm font-semibold text-slate-950">Memory</h2>
            </div>
            <div className="mt-3 space-y-2">
              {memoryRows.map((row) => (
                <Metric key={row.label} label={row.label} value={row.value} />
              ))}
            </div>
          </section>

          {selectedRun.finalOutput ? (
            <section className="rounded-lg border border-slate-200 p-4">
              <div className="text-sm font-semibold text-slate-950">Final result</div>
              <div className="mt-3 text-sm leading-6 text-slate-700">{selectedRun.finalOutput}</div>
            </section>
          ) : null}
        </aside>
      </div>
    </main>
  );
}

function BudgetField({
  label,
  value,
  onChange
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="grid gap-1 text-xs font-medium text-slate-600">
      {label}
      <input
        className="h-9 rounded-md border border-slate-200 px-2 text-sm text-slate-900 outline-none focus:border-teal-500"
        min={0}
        onChange={(event) => onChange(numberValue(event.target.value, value))}
        type="number"
        value={value}
      />
    </label>
  );
}

function Metric({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="min-w-0 rounded-md border border-slate-200 bg-slate-50 p-3">
      <div className="text-xs font-medium text-slate-500">{label}</div>
      <div className="mt-1 truncate text-sm font-semibold text-slate-900">{value}</div>
    </div>
  );
}

function StatusIcon({ status }: { status: string }) {
  if (status === "succeeded") {
    return <CheckCircle2 aria-hidden="true" className="h-4 w-4" />;
  }
  if (status === "failed" || status === "cancelled") {
    return <XCircle aria-hidden="true" className="h-4 w-4" />;
  }
  if (status === "waiting_approval" || status === "paused") {
    return <PauseCircle aria-hidden="true" className="h-4 w-4" />;
  }
  if (status === "running" || status === "resuming") {
    return <Play aria-hidden="true" className="h-4 w-4" />;
  }
  return <Clock3 aria-hidden="true" className="h-4 w-4" />;
}

function statusClass(status: string) {
  if (status === "succeeded") {
    return "bg-emerald-50 text-emerald-800 ring-1 ring-emerald-200";
  }
  if (status === "failed" || status === "cancelled") {
    return "bg-rose-50 text-rose-800 ring-1 ring-rose-200";
  }
  if (status === "waiting_approval" || status === "paused") {
    return "bg-amber-50 text-amber-800 ring-1 ring-amber-200";
  }
  if (status === "running" || status === "resuming") {
    return "bg-sky-50 text-sky-800 ring-1 ring-sky-200";
  }
  return "bg-slate-100 text-slate-700 ring-1 ring-slate-200";
}

function payloadPreview(payload: AgentRunEventResp["payload"]) {
  const text = typeof payload === "string" ? payload : JSON.stringify(payload, null, 2);
  if (!text) {
    return "-";
  }
  return text.length > 900 ? `${text.slice(0, 900)}...` : text;
}

function upsertRun(runs: AgentRunResp[], run: AgentRunResp) {
  const withoutCurrent = runs.filter((item) => item.runId !== run.runId && item.runId > 0);
  return [run, ...withoutCurrent];
}

function numberValue(value: string, fallback: number) {
  if (value === "") {
    return 0;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}
