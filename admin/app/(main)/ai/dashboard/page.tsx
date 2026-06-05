"use client";

import {
  Blocks,
  Bot,
  BrainCircuit,
  DatabaseZap,
  GitBranch,
  PackageCheck,
  PlugZap,
  RefreshCw,
  Route,
  ShieldCheck,
  Sparkles,
  Wrench
} from "lucide-react";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { getCapabilitySummary } from "@/api/ai/capability";
import { getFoundationSummary } from "@/api/ai/foundation";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { CapabilitySummaryResp } from "@/types/ai-capability";
import type { FoundationModuleResp, FoundationSummaryResp } from "@/types/ai-foundation";

const MODULE_ICONS: Record<string, typeof BrainCircuit> = {
  "novex-ai-core": GitBranch,
  "novex-model": Route,
  "novex-rag": DatabaseZap,
  "novex-agent": Bot,
  "novex-tools": Wrench,
  "novex-connectors": PlugZap,
  "novex-mcp": Blocks,
  "novex-plugin": PackageCheck,
  "novex-trigger": Sparkles,
  "novex-memory": BrainCircuit,
  "novex-eval": ShieldCheck
};

const capabilityLinks = [
  { label: "Skills", valueKey: "skillCount", href: "/ai/tools" },
  { label: "Tools", valueKey: "toolCount", href: "/ai/tools" },
  { label: "Connectors", valueKey: "connectorCount", href: "/ai/connectors" },
  { label: "Plugins", valueKey: "pluginCount", href: "/ai/plugins" },
  { label: "Triggers", valueKey: "triggerCount", href: "/ai/triggers" },
  { label: "MCP Servers", valueKey: "mcpServerCount", href: "/ai/tools" }
] as const;

const controlLinks = [
  { label: "模型路由", href: "/ai/models", permission: "ai:model:list" },
  { label: "知识库", href: "/ai/knowledge", permission: "ai:knowledge:list" },
  { label: "Agent Runs", href: "/ai/agents", permission: "ai:agent:list" },
  { label: "评测报告", href: "/ai/evals", permission: "ai:eval:report" },
  { label: "交付模板", href: "/ai/templates", permission: "ai:template:list" }
];

export default function AiDashboardPage() {
  const [foundation, setFoundation] = useState<FoundationSummaryResp | null>(null);
  const [capabilities, setCapabilities] = useState<CapabilitySummaryResp | null>(null);
  const [loading, setLoading] = useState(false);

  const loadDashboard = useCallback(async () => {
    setLoading(true);
    try {
      const [foundationSummary, capabilitySummary] = await Promise.all([
        getFoundationSummary(),
        getCapabilitySummary()
      ]);
      setFoundation(foundationSummary);
      setCapabilities(capabilitySummary);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "AI 基座总览加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDashboard();
  }, [loadDashboard]);

  const modules = foundation?.modules ?? [];
  const moduleGroups = useMemo(() => groupModulesByLayer(modules), [modules]);

  return (
    <PermissionGate permissions={["ai:foundation:read"]}>
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-4">
        <section className="rounded-lg border bg-background p-5 shadow-sm">
          <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
            <div className="min-w-0">
              <div className="inline-flex items-center gap-2 rounded-full border bg-muted/45 px-3 py-1 text-xs text-muted-foreground">
                <BrainCircuit className="size-3.5 text-primary" />
                AI Foundation
              </div>
              <h1 className="mt-3 text-xl font-semibold">AI 基座总览</h1>
              <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
                汇总 Novex 模型路由、RAG、Agent Runtime、工具治理、MCP、Eval 和客户模板的控制面状态。
              </p>
            </div>
            <Button variant="outline" size="icon" title="刷新" onClick={() => void loadDashboard()} disabled={loading}>
              <RefreshCw className={loading ? "size-4 animate-spin" : "size-4"} />
            </Button>
          </div>
        </section>

        <section className="grid gap-4 md:grid-cols-3">
          <MetricCard
            title="Foundation Modules"
            value={String(foundation?.totalModules ?? modules.length)}
            detail={foundation?.status ?? "loading"}
            icon={BrainCircuit}
          />
          <MetricCard
            title="Governed Capabilities"
            value={String(totalCapabilities(capabilities))}
            detail="skills, tools, connectors, plugins, triggers, MCP"
            icon={ShieldCheck}
          />
          <MetricCard
            title="Control Entries"
            value={String(controlLinks.length)}
            detail="models, knowledge, agent, eval, templates"
            icon={Route}
          />
        </section>

        <section className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_360px]">
          <Card className="shadow-sm">
            <CardHeader>
              <CardTitle className="text-base">模块边界</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {Object.entries(moduleGroups).map(([layer, layerModules]) => (
                <div key={layer}>
                  <div className="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    {layer}
                  </div>
                  <div className="grid gap-2 md:grid-cols-2">
                    {layerModules.map((module) => (
                      <ModuleItem key={module.id} module={module} />
                    ))}
                  </div>
                </div>
              ))}
              {!modules.length ? (
                <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
                  暂无模块摘要
                </div>
              ) : null}
            </CardContent>
          </Card>

          <div className="space-y-4">
            <Card className="shadow-sm">
              <CardHeader>
                <CardTitle className="text-base">能力治理</CardTitle>
              </CardHeader>
              <CardContent className="grid gap-2">
                {capabilityLinks.map((item) => (
                  <Link
                    className="flex items-center justify-between rounded-md border px-3 py-2 text-sm hover:bg-muted/45"
                    href={item.href}
                    key={item.label}
                  >
                    <span className="text-muted-foreground">{item.label}</span>
                    <span className="font-semibold tabular-nums">
                      {capabilities ? capabilities[item.valueKey] : "-"}
                    </span>
                  </Link>
                ))}
              </CardContent>
            </Card>

            <Card className="shadow-sm">
              <CardHeader>
                <CardTitle className="text-base">控制面入口</CardTitle>
              </CardHeader>
              <CardContent className="grid gap-2">
                {controlLinks.map((item) => (
                  <PermissionGate permissions={[item.permission]} key={item.href}>
                    <Button asChild variant="outline" className="justify-start">
                      <Link href={item.href}>{item.label}</Link>
                    </Button>
                  </PermissionGate>
                ))}
              </CardContent>
            </Card>
          </div>
        </section>
      </div>
    </PermissionGate>
  );
}

function MetricCard({
  title,
  value,
  detail,
  icon: Icon
}: {
  title: string;
  value: string;
  detail: string;
  icon: typeof BrainCircuit;
}) {
  return (
    <Card className="shadow-sm">
      <CardHeader className="relative">
        <CardTitle className="text-sm font-medium text-muted-foreground">{title}</CardTitle>
        <Icon className="app-icon absolute right-5 top-5 size-4" />
      </CardHeader>
      <CardContent>
        <div className="text-3xl font-semibold tabular-nums">{value}</div>
        <div className="mt-1 truncate text-sm text-muted-foreground">{detail}</div>
      </CardContent>
    </Card>
  );
}

function ModuleItem({ module }: { module: FoundationModuleResp }) {
  const Icon = MODULE_ICONS[module.id] ?? BrainCircuit;

  return (
    <article className="min-w-0 rounded-md border bg-muted/20 p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <Icon className="size-4 shrink-0 text-primary" />
            <div className="truncate text-sm font-semibold">{module.name}</div>
          </div>
          <div className="mt-1 truncate font-mono text-xs text-muted-foreground">{module.id}</div>
        </div>
        <Badge variant="outline" className="shrink-0">
          {module.status}
        </Badge>
      </div>
      <p className="mt-2 line-clamp-2 text-xs leading-5 text-muted-foreground">{module.description}</p>
    </article>
  );
}

function groupModulesByLayer(modules: FoundationModuleResp[]) {
  return modules.reduce<Record<string, FoundationModuleResp[]>>((groups, module) => {
    const key = module.layer || "foundation";
    groups[key] = [...(groups[key] ?? []), module];
    return groups;
  }, {});
}

function totalCapabilities(summary: CapabilitySummaryResp | null) {
  if (!summary) {
    return 0;
  }
  return (
    summary.skillCount +
    summary.toolCount +
    summary.connectorCount +
    summary.pluginCount +
    summary.triggerCount +
    summary.mcpServerCount
  );
}
