"use client";

import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Clock3,
  KeyRound,
  RefreshCw,
  ServerCog,
  SlidersHorizontal,
  XCircle
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { toast } from "sonner";
import { getModelRuntimeConfig, runModelHealthCheck } from "@/api/ai/model";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type {
  ModelHealthCheckResult,
  ModelRuntimeRouteSummary,
  ModelRuntimeSummary,
  ModelRuntimeTarget,
  ModelRoutePurpose
} from "@/types/ai-model";

const TARGET_LABELS: Record<ModelRuntimeTarget, string> = {
  llm: "LLM",
  embedding: "Embedding",
  reranker: "Reranker",
  draw: "Right Code Draw"
};

const PURPOSE_LABELS: Record<ModelRoutePurpose, string> = {
  chat: "Chat",
  rag_answer: "RAG Answer",
  query_rewrite: "Query Rewrite",
  embedding: "Embedding",
  rerank: "Rerank",
  eval_judge: "Eval Judge",
  code_agent: "Code Agent",
  media_generation: "Media Generation"
};

export default function AiModelsPage() {
  const [summary, setSummary] = useState<ModelRuntimeSummary | null>(null);
  const [healthResults, setHealthResults] = useState<ModelHealthCheckResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [checking, setChecking] = useState(false);

  const missingCount = summary?.missingEnv.length ?? 0;
  const routeCount = summary?.routes.length ?? 0;
  const healthyCount = useMemo(
    () => healthResults.filter((result) => result.ok).length,
    [healthResults]
  );

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      setSummary(await getModelRuntimeConfig());
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Model config load failed");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  async function checkAll() {
    setChecking(true);
    try {
      const result = await runModelHealthCheck({ target: "all" });
      setHealthResults(result.results);
      const ok = result.results.filter((item) => item.ok).length;
      toast.success(`${ok}/${result.results.length} model targets healthy`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Model health check failed");
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="mx-auto grid w-full max-w-7xl gap-4">
      <section className="rounded-lg border bg-background p-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <SlidersHorizontal className="size-4 text-primary" />
              <h1 className="truncate text-base font-semibold">Model Runtime</h1>
            </div>
            <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
              <span>{routeCount} Routes</span>
              <span>{missingCount} Missing Env</span>
              <span>{healthResults.length ? `${healthyCount}/${healthResults.length} Healthy` : "Not Checked"}</span>
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            <Button variant="outline" size="icon" title="Refresh" onClick={() => void loadConfig()} disabled={loading}>
              <RefreshCw />
            </Button>
            <PermissionGate permissions={["ai:model:healthCheck"]}>
              <Button onClick={() => void checkAll()} disabled={checking || loading}>
                <Activity />
                Health Check
              </Button>
            </PermissionGate>
          </div>
        </div>
      </section>

      {summary?.missingEnv.length ? (
        <section className="rounded-lg border border-destructive/40 bg-background p-4">
          <div className="mb-3 flex items-center gap-2 text-sm font-medium">
            <AlertTriangle className="size-4 text-destructive" />
            Missing Environment
          </div>
          <div className="flex flex-wrap gap-2">
            {summary.missingEnv.map((key) => (
              <Badge key={key} variant="outline" className="font-mono">
                {key}
              </Badge>
            ))}
          </div>
        </section>
      ) : null}

      <section className="grid gap-3 xl:grid-cols-2">
        {(summary?.routes ?? []).map((route) => (
          <RoutePanel key={route.routeId} route={route} />
        ))}
        {!loading && summary && !summary.routes.length ? (
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No complete model routes
          </div>
        ) : null}
        {loading && !summary ? (
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            Loading
          </div>
        ) : null}
      </section>

      <section className="rounded-lg border bg-background p-4">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div className="min-w-0">
            <h2 className="truncate text-sm font-medium">Provider Health</h2>
            <p className="text-xs text-muted-foreground">
              {healthResults.length ? `${healthyCount}/${healthResults.length} passed` : "No health check result"}
            </p>
          </div>
          <ServerCog className="size-4 text-muted-foreground" />
        </div>

        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          {healthResults.map((result) => (
            <HealthPanel key={result.target} result={result} />
          ))}
          {!healthResults.length ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground md:col-span-2 xl:col-span-4">
              Health results will appear here
            </div>
          ) : null}
        </div>
      </section>
    </div>
  );
}

function RoutePanel({ route }: { route: ModelRuntimeRouteSummary }) {
  return (
    <div className="grid min-w-0 gap-3 rounded-lg border bg-background p-4">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Badge>{TARGET_LABELS[route.target]}</Badge>
            <Badge variant="outline">{providerLabel(route.provider)}</Badge>
          </div>
          <h2 className="mt-2 truncate text-sm font-medium">{route.model ?? "No model name"}</h2>
        </div>
        <KeyRound className="size-4 shrink-0 text-muted-foreground" />
      </div>

      <div className="grid gap-2 text-xs">
        <DetailRow label="Endpoint" value={route.endpoint} mono />
        <DetailRow label="Key" value={route.maskedApiKey} mono />
        <DetailRow label="Kind" value={kindLabel(route.kind)} />
      </div>

      <div className="flex flex-wrap gap-2">
        {route.purposes.map((purpose) => (
          <Badge key={purpose} variant="secondary">
            {PURPOSE_LABELS[purpose]}
          </Badge>
        ))}
      </div>

      <div className="flex flex-wrap gap-2">
        {route.envKeys.map((key) => (
          <Badge key={key} variant="outline" className="font-mono">
            {key}
          </Badge>
        ))}
      </div>
    </div>
  );
}

function HealthPanel({ result }: { result: ModelHealthCheckResult }) {
  const statusText = result.configured
    ? result.httpStatus
      ? `HTTP ${result.httpStatus}`
      : "No response"
    : "Not configured";

  return (
    <div className="grid min-w-0 gap-3 rounded-md border p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          {result.ok ? (
            <CheckCircle2 className="size-4 shrink-0 text-primary" />
          ) : (
            <XCircle className="size-4 shrink-0 text-destructive" />
          )}
          <span className="truncate text-sm font-medium">{TARGET_LABELS[result.target]}</span>
        </div>
        <Badge variant={result.ok ? "default" : "outline"}>{result.ok ? "OK" : "Check"}</Badge>
      </div>

      <div className="grid gap-2 text-xs">
        <DetailRow label="Status" value={statusText} />
        <DetailRow label="Latency" value={`${result.latencyMs} ms`} icon={<Clock3 className="size-3.5" />} />
        <DetailRow label="Message" value={result.message} />
        {result.maskedApiKey ? <DetailRow label="Key" value={result.maskedApiKey} mono /> : null}
      </div>

      {result.detail ? <DetailSummary detail={result.detail} /> : null}
    </div>
  );
}

function DetailRow({
  label,
  value,
  mono,
  icon
}: {
  label: string;
  value: string;
  mono?: boolean;
  icon?: ReactNode;
}) {
  return (
    <div className="grid min-w-0 grid-cols-[80px_1fr] items-start gap-2">
      <span className="flex items-center gap-1 text-muted-foreground">
        {icon}
        {label}
      </span>
      <span className={["min-w-0 break-words", mono ? "font-mono" : ""].join(" ")}>{value}</span>
    </div>
  );
}

function DetailSummary({ detail }: { detail: Record<string, unknown> }) {
  return (
    <div className="flex flex-wrap gap-2">
      {Object.entries(detail).map(([key, value]) => (
        <Badge key={key} variant="outline">
          {key}: {String(value)}
        </Badge>
      ))}
    </div>
  );
}

function providerLabel(provider: ModelRuntimeRouteSummary["provider"]) {
  return provider
    .split("-")
    .map((part) => part.toUpperCase())
    .join(" ");
}

function kindLabel(kind: ModelRuntimeRouteSummary["kind"]) {
  return kind
    .split("_")
    .map((part) => part.toUpperCase())
    .join(" ");
}
