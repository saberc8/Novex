"use client";

import {
  BarChart3,
  CheckCircle2,
  Clock3,
  Database,
  ListChecks,
  Play,
  RefreshCw,
  RotateCw,
  XCircle
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  listEvalCases,
  listEvalDatasets,
  listEvalResults,
  listEvalRuns,
  runEvalDataset
} from "@/api/ai/eval";
import { listTrainingLearningRecords } from "@/api/ai/training";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from "@/components/ui/select";
import type { EvalCaseResp, EvalDatasetResp, EvalPayload, EvalResultResp, EvalRunResp } from "@/types/ai-eval";
import type { TrainingLearningRecordsResp } from "@/types/ai-training";

const DEFAULT_DATASET_CODE = "training_regression";
const TARGET_FILTERS = [
  { value: "all", label: "全部" },
  { value: "rag", label: "RAG" },
  { value: "intent", label: "Intent" },
  { value: "tool", label: "Tool" },
  { value: "react", label: "ReAct" },
  { value: "safety", label: "Safety" }
];
const BREAKDOWN_METRICS = [
  { key: "citation_accuracy", label: "RAG Citation" },
  { key: "retrieval_recall", label: "Retrieval Recall" },
  { key: "intent_accuracy", label: "Intent Accuracy" },
  { key: "tool_accuracy", label: "Tool Accuracy" },
  { key: "latency", label: "Latency" },
  { key: "cost", label: "Cost" }
];

export default function AiEvalsPage() {
  const [datasets, setDatasets] = useState<EvalDatasetResp[]>([]);
  const [cases, setCases] = useState<EvalCaseResp[]>([]);
  const [runs, setRuns] = useState<EvalRunResp[]>([]);
  const [results, setResults] = useState<EvalResultResp[]>([]);
  const [selectedDatasetCode, setSelectedDatasetCode] = useState(DEFAULT_DATASET_CODE);
  const [targetFilter, setTargetFilter] = useState("all");
  const [selectedRun, setSelectedRun] = useState<EvalRunResp | null>(null);
  const [datasetTotal, setDatasetTotal] = useState(0);
  const [caseTotal, setCaseTotal] = useState(0);
  const [runTotal, setRunTotal] = useState(0);
  const [resultTotal, setResultTotal] = useState(0);
  const [learningRecords, setLearningRecords] = useState<TrainingLearningRecordsResp | null>(null);
  const [datasetsLoading, setDatasetsLoading] = useState(false);
  const [casesLoading, setCasesLoading] = useState(false);
  const [runsLoading, setRunsLoading] = useState(false);
  const [resultsLoading, setResultsLoading] = useState(false);
  const [learningLoading, setLearningLoading] = useState(false);
  const [running, setRunning] = useState(false);

  const selectedDataset = useMemo(
    () => datasets.find((dataset) => dataset.code === selectedDatasetCode) ?? datasets[0] ?? null,
    [datasets, selectedDatasetCode]
  );

  const loadDatasets = useCallback(async () => {
    setDatasetsLoading(true);
    try {
      const result = await listEvalDatasets({ page: 1, size: 20 });
      setDatasets(result.list);
      setDatasetTotal(result.total);
      setSelectedDatasetCode((current) => {
        if (result.list.some((dataset) => dataset.code === current)) {
          return current;
        }
        return result.list[0]?.code ?? "";
      });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Eval Dataset 加载失败");
    } finally {
      setDatasetsLoading(false);
    }
  }, []);

  const loadCases = useCallback(async (datasetId?: number) => {
    const targetDatasetId = datasetId ?? selectedDataset?.id;
    if (!targetDatasetId) {
      setCases([]);
      setCaseTotal(0);
      return;
    }
    setCasesLoading(true);
    try {
      const result = await listEvalCases(targetDatasetId, {
        page: 1,
        size: 100,
        targetKind: targetFilter === "all" ? undefined : targetFilter
      });
      setCases(result.list);
      setCaseTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Eval Case 加载失败");
    } finally {
      setCasesLoading(false);
    }
  }, [selectedDataset?.id, targetFilter]);

  const loadRuns = useCallback(async (datasetCode?: string, preferredRunId?: number) => {
    const targetDatasetCode = datasetCode ?? selectedDataset?.code;
    if (!targetDatasetCode) {
      setRuns([]);
      setRunTotal(0);
      setSelectedRun(null);
      return;
    }
    setRunsLoading(true);
    try {
      const result = await listEvalRuns({ page: 1, size: 10, datasetCode: targetDatasetCode });
      setRuns(result.list);
      setRunTotal(result.total);
      setSelectedRun((current) => {
        if (!result.list.length) {
          return null;
        }
        if (preferredRunId) {
          return result.list.find((run) => run.runId === preferredRunId) ?? result.list[0];
        }
        if (current) {
          return result.list.find((run) => run.runId === current.runId) ?? result.list[0];
        }
        return result.list[0];
      });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Eval Run 加载失败");
    } finally {
      setRunsLoading(false);
    }
  }, [selectedDataset?.code]);

  const loadResults = useCallback(async (runId?: number) => {
    const targetRunId = runId ?? selectedRun?.runId;
    if (!targetRunId) {
      setResults([]);
      setResultTotal(0);
      return;
    }
    setResultsLoading(true);
    try {
      const result = await listEvalResults(targetRunId, { page: 1, size: 100 });
      setResults(result.list);
      setResultTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Eval Result 加载失败");
    } finally {
      setResultsLoading(false);
    }
  }, [selectedRun?.runId]);

  const loadLearningRecords = useCallback(async () => {
    setLearningLoading(true);
    try {
      const result = await listTrainingLearningRecords({ scope: "tenant" });
      setLearningRecords(result);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Training Learning Records 加载失败");
    } finally {
      setLearningLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDatasets();
  }, [loadDatasets]);

  useEffect(() => {
    void loadCases(selectedDataset?.id);
  }, [loadCases, selectedDataset?.id]);

  useEffect(() => {
    void loadRuns(selectedDataset?.code);
  }, [loadRuns, selectedDataset?.code]);

  useEffect(() => {
    void loadResults();
  }, [loadResults]);

  useEffect(() => {
    void loadLearningRecords();
  }, [loadLearningRecords]);

  async function runSelectedDataset() {
    if (!selectedDataset) {
      toast.error("请选择 Eval Dataset");
      return;
    }
    setRunning(true);
    try {
      const run = await runEvalDataset({
        datasetId: selectedDataset.id,
        datasetCode: selectedDataset.code
      });
      setSelectedRun(run);
      await Promise.all([loadRuns(selectedDataset.code, run.runId), loadResults(run.runId)]);
      toast.success(`Eval Run #${run.runId}`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Eval Run 创建失败");
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="mx-auto grid w-full max-w-7xl items-start gap-4 xl:grid-cols-[360px_1fr]">
      <section className="rounded-lg border bg-background p-4">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div className="min-w-0">
            <h1 className="truncate text-base font-semibold">Eval Runtime</h1>
            <p className="text-xs text-muted-foreground">{datasetTotal} Datasets</p>
          </div>
          <Button variant="outline" size="icon" title="刷新" onClick={() => void loadDatasets()} disabled={datasetsLoading}>
            <RefreshCw />
          </Button>
        </div>

        <div className="grid gap-3">
          <Select value={selectedDataset?.code ?? selectedDatasetCode} onValueChange={setSelectedDatasetCode}>
            <SelectTrigger>
              <SelectValue placeholder="选择 Dataset" />
            </SelectTrigger>
            <SelectContent>
              {datasets.map((dataset) => (
                <SelectItem key={dataset.id} value={dataset.code}>
                  {dataset.code}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <PermissionGate permissions={["ai:eval:run"]}>
            <Button onClick={() => void runSelectedDataset()} disabled={!selectedDataset || running}>
              <Play />
              Run Dataset
            </Button>
          </PermissionGate>
        </div>

        <div className="mt-4 grid gap-2">
          {datasetsLoading ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">加载中</div>
          ) : null}
          {!datasetsLoading && !datasets.length ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无 Dataset</div>
          ) : null}
          {datasets.map((dataset) => (
            <button
              key={dataset.id}
              type="button"
              className={[
                "grid w-full gap-2 rounded-md border p-3 text-left transition-colors hover:bg-muted/40",
                selectedDataset?.id === dataset.id ? "border-primary bg-muted/45" : "bg-background"
              ].join(" ")}
              onClick={() => setSelectedDatasetCode(dataset.code)}
            >
              <div className="flex min-w-0 items-center justify-between gap-2">
                <span className="truncate text-sm font-medium">{dataset.name}</span>
                <Badge variant={dataset.status === 1 ? "secondary" : "outline"}>{dataset.status === 1 ? "enabled" : "disabled"}</Badge>
              </div>
              <div className="grid gap-1 text-xs text-muted-foreground">
                <span className="truncate">{dataset.code}</span>
                <span className="inline-flex items-center gap-1">
                  <Database className="size-3.5" />
                  {dataset.caseCount} Cases
                </span>
              </div>
            </button>
          ))}
        </div>
      </section>

      <section className="grid gap-4">
        <div className="rounded-lg border bg-background p-4">
          <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div className="min-w-0">
              <h2 className="truncate text-base font-semibold">
                {selectedRun ? `Run #${selectedRun.runId}` : "Regression Report"}
              </h2>
              <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                {selectedRun ? (
                  <>
                    <Badge variant={statusVariant(selectedRun.status)}>{selectedRun.status}</Badge>
                    <span className="inline-flex items-center gap-1">
                      <ListChecks className="size-3.5" />
                      {selectedRun.datasetCode}
                    </span>
                    <span className="inline-flex items-center gap-1">
                      <Clock3 className="size-3.5" />
                      {selectedRun.finishedAt ?? selectedRun.createTime}
                    </span>
                  </>
                ) : (
                  <span>暂无 Run</span>
                )}
              </div>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                onClick={() => void loadRuns(selectedDataset?.code)}
                disabled={!selectedDataset || runsLoading}
              >
                <RotateCw />
                Runs
              </Button>
              <Button variant="outline" onClick={() => void loadResults()} disabled={!selectedRun || resultsLoading}>
                <RefreshCw />
                Results
              </Button>
            </div>
          </div>

          <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <Metric label="Average" value={selectedRun ? percent(selectedRun.averageScore) : "-"} />
            <Metric label="Passed" value={String(selectedRun?.passedCases ?? "-")} />
            <Metric label="Failed" value={String(selectedRun?.failedCases ?? "-")} />
            <Metric label="Cases" value={String(selectedRun?.totalCases ?? caseTotal)} />
          </div>

          <div className="mt-4 grid gap-3">
            {BREAKDOWN_METRICS.map((metric) => {
              const value = metricValue(selectedRun?.metricBreakdown, metric.key);
              return (
                <div key={metric.key} className="grid gap-1.5">
                  <div className="flex items-center justify-between gap-3 text-xs">
                    <span className="text-muted-foreground">{metric.label}</span>
                    <span className="font-medium">{selectedRun ? percent(value) : "-"}</span>
                  </div>
                  <div className="h-2 overflow-hidden rounded-full bg-muted">
                    <div className="h-full bg-primary" style={{ width: `${Math.round(value * 100)}%` }} />
                  </div>
                </div>
              );
            })}
          </div>

          {selectedRun ? (
            <div className="mt-4 rounded-md border bg-muted/20 p-3">
              <div className="text-xs font-medium text-muted-foreground">Regression Payload</div>
              <pre className="mt-2 max-h-36 overflow-auto whitespace-pre-wrap break-words text-xs leading-5 text-muted-foreground">
                {payloadPretty(selectedRun.reportPayload)}
              </pre>
            </div>
          ) : null}

          <div className="mt-4 border-t pt-4">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <h3 className="truncate text-sm font-medium">Training Learning Records</h3>
                <p className="text-xs text-muted-foreground">
                  {learningRecords ? `${learningRecords.records.length} Records · ${learningRecords.weakPoints.length} Weak Points` : "暂无学习记录"}
                </p>
              </div>
              <Button
                variant="outline"
                size="icon"
                title="刷新学习记录"
                onClick={() => void loadLearningRecords()}
                disabled={learningLoading}
              >
                <RefreshCw />
              </Button>
            </div>

            {learningRecords ? (
              <>
                <div className="mt-3 grid gap-2 sm:grid-cols-4">
                  <Metric label="Completion" value={`${learningRecords.summary.completionRate}%`} />
                  <Metric label="Pending" value={String(learningRecords.summary.pendingTaskCount)} />
                  <Metric label="Quiz Avg" value={String(learningRecords.summary.quizAverageScore)} />
                  <Metric label="Weak Points" value={String(learningRecords.summary.weakPointCount)} />
                </div>

                <div className="mt-3 grid gap-3 lg:grid-cols-2">
                  <div className="grid gap-2">
                    {learningRecords.records.slice(0, 4).map((record) => (
                      <div key={`${record.kind}-${record.id}`} className="rounded-md border p-3">
                        <div className="flex min-w-0 items-center justify-between gap-2">
                          <span className="truncate text-sm font-medium">{record.title}</span>
                          <Badge variant={record.status === "needs_review" ? "destructive" : "outline"}>{record.status}</Badge>
                        </div>
                        <div className="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
                          <span>{record.learnerName}</span>
                          <span>{record.createTime}</span>
                          {typeof record.score === "number" ? <span>{percent(record.score)}</span> : null}
                        </div>
                        <p className="mt-2 line-clamp-2 text-xs text-muted-foreground">{record.detail}</p>
                      </div>
                    ))}
                  </div>

                  <div className="grid content-start gap-2">
                    {learningRecords.weakPoints.slice(0, 4).map((weakPoint) => (
                      <div key={`${weakPoint.topic}-${weakPoint.evidence}`} className="rounded-md border p-3">
                        <div className="flex min-w-0 items-center justify-between gap-2">
                          <span className="truncate text-sm font-medium">{weakPoint.topic}</span>
                          <Badge variant="outline">{weakPoint.count}</Badge>
                        </div>
                        <div className="mt-1 text-xs text-muted-foreground">
                          {weakPoint.evidence} · {weakPoint.lastSeenAt}
                        </div>
                      </div>
                    ))}
                    {!learningRecords.weakPoints.length ? (
                      <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无薄弱点</div>
                    ) : null}
                  </div>
                </div>
              </>
            ) : null}
          </div>
        </div>

        <div className="grid gap-4 lg:grid-cols-[0.9fr_1.1fr]">
          <div className="rounded-lg border bg-background p-4">
            <div className="mb-3 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <h2 className="truncate text-sm font-medium">Eval Cases</h2>
                <p className="text-xs text-muted-foreground">{caseTotal} Cases</p>
              </div>
              <Select value={targetFilter} onValueChange={setTargetFilter}>
                <SelectTrigger className="w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {TARGET_FILTERS.map((target) => (
                    <SelectItem key={target.value} value={target.value}>
                      {target.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="grid gap-2">
              {casesLoading ? (
                <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">加载中</div>
              ) : null}
              {!casesLoading && !cases.length ? (
                <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无 Case</div>
              ) : null}
              {cases.map((item) => (
                <div key={item.id} className="grid gap-2 rounded-md border p-3">
                  <div className="flex min-w-0 items-center justify-between gap-2">
                    <span className="truncate text-sm font-medium">{item.caseCode}</span>
                    <Badge variant="outline">{item.targetKind}</Badge>
                  </div>
                  <p className="line-clamp-2 text-xs text-muted-foreground">{item.prompt}</p>
                  <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                    <span>{item.metricKind}</span>
                    <span>Sort {item.sort}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="rounded-lg border bg-background p-4">
            <div className="mb-3 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <h2 className="truncate text-sm font-medium">Eval Results</h2>
                <p className="text-xs text-muted-foreground">{resultTotal} Results</p>
              </div>
              <div className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                <BarChart3 className="size-3.5" />
                {runTotal} Runs
              </div>
            </div>

            <div className="grid gap-2">
              {resultsLoading ? (
                <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">加载中</div>
              ) : null}
              {!resultsLoading && !results.length ? (
                <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无 Result</div>
              ) : null}
              {results.map((result) => (
                <button
                  key={result.id}
                  type="button"
                  className="grid w-full gap-2 rounded-md border p-3 text-left transition-colors hover:bg-muted/40"
                  onClick={() => {
                    const run = runs.find((item) => item.runId === result.runId);
                    if (run) {
                      setSelectedRun(run);
                    }
                  }}
                >
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-2">
                      {result.passed ? (
                        <CheckCircle2 className="size-4 text-primary" />
                      ) : (
                        <XCircle className="size-4 text-destructive" />
                      )}
                      <span className="truncate text-sm font-medium">{result.caseCode}</span>
                    </div>
                    <Badge variant={result.passed ? "secondary" : "destructive"}>{percent(result.score)}</Badge>
                  </div>
                  <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                    <span>{result.targetKind}</span>
                    <span>{result.metricKind}</span>
                    <span>{result.latencyMs}ms</span>
                    <span>{result.costCents}c</span>
                  </div>
                  <p className="line-clamp-2 text-xs text-muted-foreground">{result.reason || payloadPreview(result.actualPayload)}</p>
                </button>
              ))}
            </div>
          </div>
        </div>
      </section>
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

function metricValue(payload: EvalPayload | undefined, key: string) {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    return 0;
  }
  const value = payload[key];
  return typeof value === "number" && Number.isFinite(value) ? clamp01(value) : 0;
}

function clamp01(value: number) {
  return Math.max(0, Math.min(1, value));
}

function percent(value: number) {
  return `${Math.round(clamp01(value) * 100)}%`;
}

function payloadPreview(payload: EvalPayload) {
  const text = typeof payload === "string" ? payload : JSON.stringify(payload);
  if (!text) {
    return "-";
  }
  return text.length > 180 ? `${text.slice(0, 180)}...` : text;
}

function payloadPretty(payload: EvalPayload) {
  if (payload === null || payload === undefined) {
    return "-";
  }
  if (typeof payload === "string") {
    return payload || "-";
  }
  return JSON.stringify(payload, null, 2);
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
