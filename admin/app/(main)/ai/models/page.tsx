"use client";

import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Database,
  KeyRound,
  Pencil,
  Plus,
  RefreshCw,
  Save,
  ServerCog,
  SlidersHorizontal,
  Trash2,
  XCircle
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import type { ReactNode } from "react";
import { toast } from "sonner";
import {
  getModelRegistry,
  getModelRuntimeConfig,
  runModelHealthCheck,
  upsertModelRegistryRoute,
  deleteModelRegistryRoute
} from "@/api/ai/model";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from "@/components/ui/select";
import type {
  ModelHealthCheckResult,
  ModelRegistryRouteCommand,
  ModelRegistrySummary,
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
  guardian_review: "Guardian Review",
  media_generation: "Media Generation"
};

type RegistryRoute = ModelRegistrySummary["routes"][number];

const DEFAULT_ROUTE_COMMAND: ModelRegistryRouteCommand = {
  providerCode: "deepseek",
  providerName: "DeepSeek",
  providerType: "deep-seek",
  protocol: "openai-compatible",
  deploymentCode: "deepseek-public",
  deploymentName: "DeepSeek Public API",
  endpoint: "https://api.deepseek.com",
  apiPath: "/chat/completions",
  networkZone: "public",
  timeoutMs: 20000,
  maxConcurrency: null,
  profileCode: "deepseek-v4-flash",
  profileName: "DeepSeek V4 Flash",
  modelName: "deepseek-v4-flash",
  modelKind: "llm",
  credentialCode: "env-llm-api-key",
  credentialRef: "env:LLM_API_KEY",
  routeCode: "runtime.llm.chat",
  routePurpose: "chat",
  priority: 100,
  status: 1
};

export default function AiModelsPage() {
  const [summary, setSummary] = useState<ModelRuntimeSummary | null>(null);
  const [registry, setRegistry] = useState<ModelRegistrySummary | null>(null);
  const [healthResults, setHealthResults] = useState<ModelHealthCheckResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [checking, setChecking] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [editingRouteId, setEditingRouteId] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [deletingRouteId, setDeletingRouteId] = useState<number | null>(null);
  const [form, setForm] = useState<ModelRegistryRouteCommand>(DEFAULT_ROUTE_COMMAND);

  const missingCount = summary?.missingEnv.length ?? 0;
  const routeCount = summary?.routes.length ?? 0;
  const dbRouteCount = registry?.routeCount ?? 0;
  const healthyCount = useMemo(
    () => healthResults.filter((result) => result.ok).length,
    [healthResults]
  );

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      const [runtimeSummary, registrySummary] = await Promise.all([
        getModelRuntimeConfig(),
        getModelRegistry()
      ]);
      setSummary(runtimeSummary);
      setRegistry(registrySummary);
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

  async function submitModel(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const command = normalizeRouteCommandForm(form);
    if (
      !command.providerCode ||
      !command.deploymentCode ||
      !command.profileCode ||
      !command.modelName ||
      !command.routeCode
    ) {
      toast.error("请填写模型和路由");
      return;
    }
    if (!editingRouteId && !command.credentialRef) {
      toast.error("新增模型必须填写 env 引用");
      return;
    }

    setSaving(true);
    try {
      await upsertModelRegistryRoute(command);
      await loadConfig();
      closeModelDialog();
      toast.success("模型路由已保存");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "模型路由保存失败");
    } finally {
      setSaving(false);
    }
  }

  function openCreateModel() {
    setEditingRouteId(null);
    setForm(DEFAULT_ROUTE_COMMAND);
    setCreateOpen(true);
  }

  function openEditModel(route: ModelRegistrySummary["routes"][number]) {
    if (!registry) {
      return;
    }
    setEditingRouteId(route.id);
    setForm(commandFromRegistryRoute(route, registry));
    setCreateOpen(true);
  }

  function closeModelDialog() {
    setCreateOpen(false);
    setEditingRouteId(null);
    setForm(DEFAULT_ROUTE_COMMAND);
  }

  async function deleteRoute(route: ModelRegistrySummary["routes"][number]) {
    if (!window.confirm(`删除模型路由 ${route.code}？`)) {
      return;
    }
    setDeletingRouteId(route.id);
    try {
      await deleteModelRegistryRoute(route.id);
      await loadConfig();
      toast.success("模型路由已删除");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "模型路由删除失败");
    } finally {
      setDeletingRouteId(null);
    }
  }

  return (
    <div className="mx-auto grid min-w-0 w-full max-w-7xl gap-4">
      <section className="min-w-0 rounded-lg border bg-background p-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <SlidersHorizontal className="size-4 text-primary" />
              <h1 className="truncate text-base font-semibold">Model Runtime</h1>
            </div>
            <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
              <span>{routeCount} Routes</span>
              <span>{dbRouteCount} DB Routes</span>
              <span>{missingCount} Missing Env</span>
              <span>{healthResults.length ? `${healthyCount}/${healthResults.length} Healthy` : "Not Checked"}</span>
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            <Button variant="outline" size="icon" title="Refresh" onClick={() => void loadConfig()} disabled={loading}>
              <RefreshCw />
            </Button>
            <PermissionGate permissions={["ai:model:manage"]}>
              <Button variant="outline" onClick={openCreateModel} disabled={loading}>
                <Plus />
                新增模型
              </Button>
            </PermissionGate>
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
        <section className="min-w-0 rounded-lg border border-destructive/40 bg-background p-4">
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

      <section className="min-w-0 rounded-lg border bg-background p-4">
        <div className="mb-3 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Database className="size-4 text-primary" />
              <h2 className="truncate text-sm font-medium">Database Registry</h2>
            </div>
            <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
              <span>{registry?.providerCount ?? 0} Providers</span>
              <span>{registry?.deploymentCount ?? 0} Deployments</span>
              <span>{registry?.profileCount ?? 0} Profiles</span>
              <span>{registry?.routeCount ?? 0} Routes</span>
            </div>
          </div>
          <Badge variant="outline">ai_model_route</Badge>
        </div>

        <div className="min-w-0 overflow-hidden rounded-md border">
          {registry ? (
            <RegistryRouteTable
              registry={registry}
              deletingRouteId={deletingRouteId}
              onEdit={openEditModel}
              onDelete={(route) => void deleteRoute(route)}
            />
          ) : null}
          {!loading && registry && !registry.routes.length ? (
            <div className="border-t border-dashed p-8 text-center text-sm text-muted-foreground">
              No database model routes
            </div>
          ) : null}
        </div>
      </section>

      <section className="grid min-w-0 gap-3 xl:grid-cols-2">
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

      <section className="min-w-0 rounded-lg border bg-background p-4">
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

      <Dialog open={createOpen} onOpenChange={(open) => (open ? setCreateOpen(true) : closeModelDialog())}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle>{editingRouteId ? "编辑模型" : "新增模型"}</DialogTitle>
          </DialogHeader>
          <ModelRouteForm
            form={form}
            editing={!!editingRouteId}
            saving={saving}
            onChange={setForm}
            onSubmit={submitModel}
            onCancel={closeModelDialog}
          />
        </DialogContent>
      </Dialog>
    </div>
  );
}

function RegistryRouteTable({
  registry,
  deletingRouteId,
  onEdit,
  onDelete
}: {
  registry: ModelRegistrySummary;
  deletingRouteId: number | null;
  onEdit: (route: RegistryRoute) => void;
  onDelete: (route: RegistryRoute) => void;
}) {
  if (!registry.routes.length) {
    return null;
  }

  return (
    <>
      <div className="divide-y lg:hidden">
        {registry.routes.map((route) => (
          <RegistryRouteCompactRow
            key={route.id}
            route={route}
            registry={registry}
            deletingRouteId={deletingRouteId}
            onEdit={onEdit}
            onDelete={onDelete}
          />
        ))}
      </div>
      <div className="hidden lg:block">
      <table className="w-full min-w-full table-fixed border-collapse text-left text-sm">
        <thead className="bg-muted/50 text-xs text-muted-foreground">
          <tr className="[&>th]:px-3 [&>th]:py-2">
            <th className="w-[18%] font-medium">Route</th>
            <th className="w-[19%] font-medium">Model</th>
            <th className="w-[15%] font-medium">Provider</th>
            <th className="w-[23%] font-medium">Endpoint</th>
            <th className="w-[10%] font-medium">Credential</th>
            <th className="w-[10%] font-medium">Policy</th>
            <th className="w-[5%] text-right font-medium">Actions</th>
          </tr>
        </thead>
        <tbody className="divide-y">
          {registry.routes.map((route) => {
            const { profile, deployment, provider } = registryRouteContext(route, registry);
            const endpoint = deployment
              ? displayEndpoint(deployment.endpoint, deployment.apiPath)
              : "-";

            return (
              <tr key={route.id} className="align-middle hover:bg-muted/35">
                <td className="px-3 py-2.5">
                  <div className="truncate font-mono text-xs font-medium">{route.code}</div>
                  <div className="mt-1 flex flex-wrap items-center gap-1.5">
                    <PurposeTag purpose={route.routePurpose} />
                    <StatusTag status={route.status} />
                  </div>
                </td>
                <td className="px-3 py-2.5">
                  <div className="flex min-w-0 items-center gap-2">
                    <KindTag kind={profile?.modelKind} />
                    <span className="truncate font-medium">{profile?.modelName ?? "-"}</span>
                  </div>
                  <div className="mt-1 truncate font-mono text-xs text-muted-foreground">
                    {profile?.code ?? "-"}
                  </div>
                </td>
                <td className="px-3 py-2.5">
                  <div className="flex min-w-0 items-center gap-2">
                    <ProviderTag providerType={provider?.providerType} />
                    <span className="truncate">{provider?.name ?? "-"}</span>
                  </div>
                  <div className="mt-1 truncate font-mono text-xs text-muted-foreground">
                    {deployment?.code ?? "-"}
                  </div>
                </td>
                <td className="px-3 py-2.5">
                  <div className="truncate font-mono text-xs">{endpoint}</div>
                  <div className="mt-1 flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
                    <span>{deployment?.networkZone ?? route.policyStatus.networkZone}</span>
                    {deployment?.apiPath ? <span className="font-mono">{deployment.apiPath}</span> : null}
                  </div>
                </td>
                <td className="px-3 py-2.5">
                  <span className="inline-flex max-w-[116px] items-center rounded border border-emerald-200 bg-emerald-50 px-2 py-0.5 font-mono text-xs text-emerald-700">
                    <span className="truncate">{route.maskedCredential ?? "未配置"}</span>
                  </span>
                </td>
                <td className="px-3 py-2.5">
                  <div className="flex flex-wrap items-center gap-1.5">
                    <span className="rounded border border-slate-200 bg-slate-50 px-2 py-0.5 text-xs text-slate-700">
                      P{route.priority}
                    </span>
                    <span className="rounded border border-cyan-200 bg-cyan-50 px-2 py-0.5 text-xs text-cyan-700">
                      {route.policyStatus.networkZone}
                    </span>
                    {route.fallbackRouteId ? (
                      <span className="rounded border border-violet-200 bg-violet-50 px-2 py-0.5 text-xs text-violet-700">
                        Fallback
                      </span>
                    ) : null}
                  </div>
                  {route.policyStatus.violations.length ? (
                    <div className="mt-1 flex flex-wrap gap-1">
                      {route.policyStatus.violations.map((violation) => (
                        <Badge key={violation} variant="destructive" className="text-[11px]">
                          {violation}
                        </Badge>
                      ))}
                    </div>
                  ) : null}
                </td>
                <td className="px-3 py-2.5 text-right">
                  <PermissionGate permissions={["ai:model:manage"]}>
                    <div className="flex justify-end gap-1">
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        title={`编辑 ${route.code}`}
                        aria-label={`编辑 ${route.code}`}
                        onClick={() => onEdit(route)}
                      >
                        <Pencil className="size-4" />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        title={`删除 ${route.code}`}
                        aria-label={`删除 ${route.code}`}
                        className="text-destructive hover:text-destructive"
                        disabled={deletingRouteId === route.id}
                        onClick={() => onDelete(route)}
                      >
                        <Trash2 className="size-4" />
                      </Button>
                    </div>
                  </PermissionGate>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      </div>
    </>
  );
}

function RegistryRouteCompactRow({
  route,
  registry,
  deletingRouteId,
  onEdit,
  onDelete
}: {
  route: RegistryRoute;
  registry: ModelRegistrySummary;
  deletingRouteId: number | null;
  onEdit: (route: RegistryRoute) => void;
  onDelete: (route: RegistryRoute) => void;
}) {
  const { profile, deployment, provider } = registryRouteContext(route, registry);
  const endpoint = deployment ? displayEndpoint(deployment.endpoint, deployment.apiPath) : "-";

  return (
    <div className="grid min-w-0 gap-2 p-3">
      <div className="flex min-w-0 items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate font-mono text-xs font-medium">{route.code}</div>
          <div className="mt-1 flex flex-wrap items-center gap-1.5">
            <PurposeTag purpose={route.routePurpose} />
            <KindTag kind={profile?.modelKind} />
            <StatusTag status={route.status} />
          </div>
        </div>
        <PermissionGate permissions={["ai:model:manage"]}>
          <div className="flex shrink-0 gap-1">
            <Button
              type="button"
              variant="ghost"
              size="icon"
              title={`编辑 ${route.code}`}
              aria-label={`编辑 ${route.code}`}
              onClick={() => onEdit(route)}
            >
              <Pencil className="size-4" />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              title={`删除 ${route.code}`}
              aria-label={`删除 ${route.code}`}
              className="text-destructive hover:text-destructive"
              disabled={deletingRouteId === route.id}
              onClick={() => onDelete(route)}
            >
              <Trash2 className="size-4" />
            </Button>
          </div>
        </PermissionGate>
      </div>

      <div className="grid min-w-0 grid-cols-1 gap-2 text-xs sm:grid-cols-2">
        <div className="min-w-0">
          <div className="text-muted-foreground">Model</div>
          <div className="truncate font-medium">{profile?.modelName ?? "-"}</div>
        </div>
        <div className="min-w-0">
          <div className="text-muted-foreground">Provider</div>
          <div className="flex min-w-0 items-center gap-1.5">
            <ProviderTag providerType={provider?.providerType} />
            <span className="truncate">{provider?.name ?? "-"}</span>
          </div>
        </div>
        <div className="col-span-1 min-w-0 sm:col-span-2">
          <div className="text-muted-foreground">Endpoint</div>
          <div className="truncate font-mono">{endpoint}</div>
        </div>
        <div className="min-w-0">
          <div className="text-muted-foreground">Credential</div>
          <div className="truncate font-mono">{route.maskedCredential ?? "未配置"}</div>
        </div>
        <div className="min-w-0">
          <div className="text-muted-foreground">Policy</div>
          <div className="truncate">
            P{route.priority} · {route.policyStatus.networkZone}
          </div>
        </div>
      </div>
    </div>
  );
}

function ModelRouteForm({
  form,
  editing,
  saving,
  onChange,
  onSubmit,
  onCancel
}: {
  form: ModelRegistryRouteCommand;
  editing: boolean;
  saving: boolean;
  onChange: (form: ModelRegistryRouteCommand) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onCancel: () => void;
}) {
  return (
    <form className="grid gap-4" onSubmit={onSubmit}>
      <div className="grid gap-3 md:grid-cols-3">
        <FormField id="provider-code" label="供应商编码">
          <Input
            id="provider-code"
            value={form.providerCode}
            onChange={(event) => onChange({ ...form, providerCode: event.target.value })}
          />
        </FormField>
        <FormField id="provider-name" label="供应商名称">
          <Input
            id="provider-name"
            value={form.providerName ?? ""}
            onChange={(event) => onChange({ ...form, providerName: event.target.value })}
          />
        </FormField>
        <FormField id="provider-type" label="供应商类型">
          <Select
            value={form.providerType}
            onValueChange={(providerType) => onChange({ ...form, providerType })}
          >
            <SelectTrigger id="provider-type">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="deep-seek">deep-seek</SelectItem>
              <SelectItem value="dash-scope">dash-scope</SelectItem>
              <SelectItem value="openai-compatible">openai-compatible</SelectItem>
              <SelectItem value="local-runtime">local-runtime</SelectItem>
              <SelectItem value="right-code-draw">right-code-draw</SelectItem>
            </SelectContent>
          </Select>
        </FormField>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <FormField id="deployment-code" label="部署编码">
          <Input
            id="deployment-code"
            value={form.deploymentCode}
            onChange={(event) => onChange({ ...form, deploymentCode: event.target.value })}
          />
        </FormField>
        <FormField id="deployment-name" label="部署名称">
          <Input
            id="deployment-name"
            value={form.deploymentName ?? ""}
            onChange={(event) => onChange({ ...form, deploymentName: event.target.value })}
          />
        </FormField>
      </div>

      <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_220px]">
        <FormField id="endpoint" label="Endpoint">
          <Input
            id="endpoint"
            value={form.endpoint}
            onChange={(event) => onChange({ ...form, endpoint: event.target.value })}
          />
        </FormField>
        <FormField id="api-path" label="API Path">
          <Input
            id="api-path"
            value={form.apiPath ?? ""}
            onChange={(event) => onChange({ ...form, apiPath: event.target.value })}
          />
        </FormField>
      </div>

      <div className="grid gap-3 md:grid-cols-3">
        <FormField id="profile-code" label="Profile 编码">
          <Input
            id="profile-code"
            value={form.profileCode}
            onChange={(event) => onChange({ ...form, profileCode: event.target.value })}
          />
        </FormField>
        <FormField id="model-name" label="模型名称">
          <Input
            id="model-name"
            value={form.modelName}
            onChange={(event) => onChange({ ...form, modelName: event.target.value })}
          />
        </FormField>
        <FormField id="model-kind" label="模型类型">
          <Select value={form.modelKind} onValueChange={(modelKind) => onChange({ ...form, modelKind })}>
            <SelectTrigger id="model-kind">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="llm">llm</SelectItem>
              <SelectItem value="embedding">embedding</SelectItem>
              <SelectItem value="rerank">rerank</SelectItem>
              <SelectItem value="media_generation">media_generation</SelectItem>
              <SelectItem value="vlm">vlm</SelectItem>
              <SelectItem value="asr">asr</SelectItem>
              <SelectItem value="tts">tts</SelectItem>
            </SelectContent>
          </Select>
        </FormField>
      </div>

      <div className="grid gap-3 md:grid-cols-3">
        <FormField id="credential-code" label="凭据编码">
          <Input
            id="credential-code"
            value={form.credentialCode ?? ""}
            onChange={(event) => onChange({ ...form, credentialCode: event.target.value })}
          />
        </FormField>
        <FormField id="credential-ref" label="环境变量引用">
          <Input
            id="credential-ref"
            value={form.credentialRef ?? ""}
            placeholder={editing ? "留空保持当前 env 引用" : "env:LLM_API_KEY"}
            onChange={(event) => onChange({ ...form, credentialRef: event.target.value })}
          />
        </FormField>
        <FormField id="network-zone" label="网络区域">
          <Input
            id="network-zone"
            value={form.networkZone ?? "public"}
            onChange={(event) => onChange({ ...form, networkZone: event.target.value })}
          />
        </FormField>
      </div>

      <div className="grid gap-3 md:grid-cols-4">
        <FormField id="route-code" label="路由编码">
          <Input
            id="route-code"
            value={form.routeCode}
            onChange={(event) => onChange({ ...form, routeCode: event.target.value })}
          />
        </FormField>
        <FormField id="route-purpose" label="路由用途">
          <Select
            value={form.routePurpose}
            onValueChange={(routePurpose) => onChange({ ...form, routePurpose })}
          >
            <SelectTrigger id="route-purpose">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {Object.entries(PURPOSE_LABELS).map(([value, label]) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </FormField>
        <FormField id="priority" label="优先级">
          <Input
            id="priority"
            type="number"
            min={1}
            value={form.priority ?? ""}
            onChange={(event) => onChange({ ...form, priority: numberOrNull(event.target.value) })}
          />
        </FormField>
        <FormField id="timeout-ms" label="超时毫秒">
          <Input
            id="timeout-ms"
            type="number"
            min={1}
            value={form.timeoutMs ?? ""}
            onChange={(event) => onChange({ ...form, timeoutMs: numberOrNull(event.target.value) })}
          />
        </FormField>
      </div>

      <div className="flex justify-end gap-2">
        <Button type="button" variant="outline" onClick={onCancel} disabled={saving}>
          取消
        </Button>
        <Button type="submit" disabled={saving}>
          <Save />
          保存模型
        </Button>
      </div>
    </form>
  );
}

function FormField({
  id,
  label,
  children
}: {
  id: string;
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
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

function routePurposeLabel(purpose: string) {
  return PURPOSE_LABELS[purpose as ModelRoutePurpose] ?? purpose;
}

function registryRouteContext(route: RegistryRoute, registry: ModelRegistrySummary) {
  const profile = registry.profiles.find((item) => item.id === route.modelProfileId);
  const deployment = profile
    ? registry.deployments.find((item) => item.id === profile.deploymentId)
    : undefined;
  const provider = deployment
    ? registry.providers.find((item) => item.id === deployment.providerId)
    : undefined;

  return { profile, deployment, provider };
}

function commandFromRegistryRoute(
  route: RegistryRoute,
  registry: ModelRegistrySummary
): ModelRegistryRouteCommand {
  const { profile, deployment, provider } = registryRouteContext(route, registry);

  return {
    ...DEFAULT_ROUTE_COMMAND,
    providerCode: provider?.code ?? DEFAULT_ROUTE_COMMAND.providerCode,
    providerName: provider?.name ?? provider?.code ?? DEFAULT_ROUTE_COMMAND.providerName,
    providerType: provider?.providerType ?? DEFAULT_ROUTE_COMMAND.providerType,
    deploymentCode: deployment?.code ?? DEFAULT_ROUTE_COMMAND.deploymentCode,
    deploymentName: deployment?.name ?? deployment?.code ?? DEFAULT_ROUTE_COMMAND.deploymentName,
    endpoint: deployment?.endpoint ?? DEFAULT_ROUTE_COMMAND.endpoint,
    apiPath: deployment?.apiPath ?? null,
    networkZone: deployment?.networkZone ?? DEFAULT_ROUTE_COMMAND.networkZone,
    profileCode: profile?.code ?? DEFAULT_ROUTE_COMMAND.profileCode,
    profileName: profile?.name ?? profile?.code ?? DEFAULT_ROUTE_COMMAND.profileName,
    modelName: profile?.modelName ?? DEFAULT_ROUTE_COMMAND.modelName,
    modelKind: profile?.modelKind ?? DEFAULT_ROUTE_COMMAND.modelKind,
    credentialCode: null,
    credentialRef: null,
    routeCode: route.code,
    routePurpose: route.routePurpose,
    priority: route.priority,
    status: route.status
  };
}

function displayEndpoint(endpoint: string, apiPath: string | null | undefined) {
  const base = endpoint.trim().replace(/\/+$/, "");
  const path = apiPath?.trim().replace(/^\/+|\/+$/g, "");
  return path ? `${base}/${path}` : base;
}

const TAG_BASE_CLASS =
  "inline-flex max-w-full items-center overflow-hidden text-ellipsis whitespace-nowrap rounded border px-2 py-0.5 text-[11px] font-medium leading-4";

const KIND_TAG_CLASSES: Record<string, string> = {
  llm: "border-sky-200 bg-sky-50 text-sky-700",
  embedding: "border-emerald-200 bg-emerald-50 text-emerald-700",
  rerank: "border-amber-200 bg-amber-50 text-amber-700",
  media_generation: "border-fuchsia-200 bg-fuchsia-50 text-fuchsia-700",
  vlm: "border-violet-200 bg-violet-50 text-violet-700",
  asr: "border-cyan-200 bg-cyan-50 text-cyan-700",
  tts: "border-rose-200 bg-rose-50 text-rose-700",
  default: "border-slate-200 bg-slate-50 text-slate-700"
};

const PURPOSE_TAG_CLASSES: Record<string, string> = {
  chat: "border-indigo-200 bg-indigo-50 text-indigo-700",
  rag_answer: "border-teal-200 bg-teal-50 text-teal-700",
  query_rewrite: "border-lime-200 bg-lime-50 text-lime-700",
  embedding: "border-emerald-200 bg-emerald-50 text-emerald-700",
  rerank: "border-amber-200 bg-amber-50 text-amber-700",
  eval_judge: "border-orange-200 bg-orange-50 text-orange-700",
  code_agent: "border-blue-200 bg-blue-50 text-blue-700",
  guardian_review: "border-rose-200 bg-rose-50 text-rose-700",
  media_generation: "border-fuchsia-200 bg-fuchsia-50 text-fuchsia-700",
  default: "border-slate-200 bg-slate-50 text-slate-700"
};

const PROVIDER_TAG_CLASSES: Record<string, string> = {
  "deep-seek": "border-purple-200 bg-purple-50 text-purple-700",
  "dash-scope": "border-amber-200 bg-amber-50 text-amber-700",
  "open-ai-compatible": "border-green-200 bg-green-50 text-green-700",
  "openai-compatible": "border-green-200 bg-green-50 text-green-700",
  "azure-open-ai": "border-cyan-200 bg-cyan-50 text-cyan-700",
  "azure-openai": "border-cyan-200 bg-cyan-50 text-cyan-700",
  "local-runtime": "border-zinc-200 bg-zinc-50 text-zinc-700",
  "right-code-draw": "border-pink-200 bg-pink-50 text-pink-700",
  default: "border-slate-200 bg-slate-50 text-slate-700"
};

function KindTag({ kind }: { kind: string | null | undefined }) {
  const value = kind ?? "unknown";
  return <ColorTag className={tagClass(KIND_TAG_CLASSES, kind)}>{kindLabelText(value)}</ColorTag>;
}

function PurposeTag({ purpose }: { purpose: string }) {
  return (
    <ColorTag className={tagClass(PURPOSE_TAG_CLASSES, purpose)}>
      {routePurposeLabel(purpose)}
    </ColorTag>
  );
}

function ProviderTag({ providerType }: { providerType: string | null | undefined }) {
  const value = providerType ?? "unknown";
  return <ColorTag className={tagClass(PROVIDER_TAG_CLASSES, providerType)}>{value}</ColorTag>;
}

function StatusTag({ status }: { status: number }) {
  const className =
    status === 1
      ? "border-emerald-200 bg-emerald-50 text-emerald-700"
      : "border-stone-200 bg-stone-50 text-stone-700";
  return <ColorTag className={className}>{status === 1 ? "Enabled" : "Disabled"}</ColorTag>;
}

function ColorTag({ className, children }: { className: string; children: ReactNode }) {
  return <span className={[TAG_BASE_CLASS, className].join(" ")}>{children}</span>;
}

function tagClass(classMap: Record<string, string>, value: string | null | undefined) {
  return classMap[value ?? ""] ?? classMap.default;
}

function kindLabelText(kind: string) {
  return kind
    .split("_")
    .map((part) => part.toUpperCase())
    .join(" ");
}

function numberOrNull(value: string) {
  const trimmed = value.trim();
  return trimmed ? Number(trimmed) : null;
}

function trimOptional(value: string | null | undefined) {
  const trimmed = value?.trim() ?? "";
  return trimmed || null;
}

function normalizeRouteCommandForm(form: ModelRegistryRouteCommand): ModelRegistryRouteCommand {
  return {
    ...form,
    providerCode: form.providerCode.trim(),
    providerName: trimOptional(form.providerName),
    providerType: form.providerType.trim(),
    protocol: trimOptional(form.protocol),
    deploymentCode: form.deploymentCode.trim(),
    deploymentName: trimOptional(form.deploymentName),
    endpoint: form.endpoint.trim(),
    apiPath: trimOptional(form.apiPath),
    networkZone: trimOptional(form.networkZone),
    timeoutMs: form.timeoutMs ? Number(form.timeoutMs) : null,
    maxConcurrency: form.maxConcurrency ? Number(form.maxConcurrency) : null,
    profileCode: form.profileCode.trim(),
    profileName: trimOptional(form.profileName),
    modelName: form.modelName.trim(),
    modelKind: form.modelKind.trim(),
    credentialCode: trimOptional(form.credentialCode),
    credentialRef: trimOptional(form.credentialRef),
    routeCode: form.routeCode.trim(),
    routePurpose: form.routePurpose.trim(),
    priority: form.priority ? Number(form.priority) : null,
    status: form.status ? Number(form.status) : 1
  };
}
