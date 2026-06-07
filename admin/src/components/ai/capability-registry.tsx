"use client";

import type { ColumnDef } from "@tanstack/react-table";
import { PackageCheck, Play, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  dryRunTool,
  installPlugin,
  listConnectors,
  listMcpServers,
  listPluginInstallations,
  listPlugins,
  listSkills,
  listToolAudits,
  listTools,
  listTriggers
} from "@/api/ai/capability";
import { PermissionGate } from "@/components/permission/permission-gate";
import { DataTable } from "@/components/table/data-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { PageResult } from "@/types/api";
import type { CapabilityItemResp, PluginInstallationResp, ToolCallAuditResp } from "@/types/ai-capability";

type CapabilityResource = "skills" | "tools" | "connectors" | "plugins" | "triggers" | "mcpServers";

interface CapabilityRegistryProps {
  title: string;
  resource: CapabilityResource;
  permission: string;
}

const LOADERS: Record<
  CapabilityResource,
  () => Promise<PageResult<CapabilityItemResp>>
> = {
  skills: () => listSkills({ page: 1, size: 50 }),
  tools: () => listTools({ page: 1, size: 50 }),
  connectors: () => listConnectors({ page: 1, size: 50 }),
  plugins: () => listPlugins({ page: 1, size: 50 }),
  triggers: () => listTriggers({ page: 1, size: 50 }),
  mcpServers: () => listMcpServers({ page: 1, size: 50 })
};

export function CapabilityRegistry({ title, resource, permission }: CapabilityRegistryProps) {
  const [items, setItems] = useState<CapabilityItemResp[]>([]);
  const [total, setTotal] = useState(0);
  const [audits, setAudits] = useState<ToolCallAuditResp[]>([]);
  const [pluginInstallations, setPluginInstallations] = useState<PluginInstallationResp[]>([]);
  const [loading, setLoading] = useState(false);
  const [auditLoading, setAuditLoading] = useState(false);
  const [runningToolCode, setRunningToolCode] = useState<string | null>(null);
  const [installingPluginCode, setInstallingPluginCode] = useState<string | null>(null);

  const loadItems = useCallback(async () => {
    setLoading(true);
    try {
      const result = await LOADERS[resource]();
      setItems(result.list);
      setTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "能力列表加载失败");
    } finally {
      setLoading(false);
    }
  }, [resource]);

  const loadAudits = useCallback(async () => {
    if (resource !== "tools") {
      setAudits([]);
      return;
    }
    setAuditLoading(true);
    try {
      const result = await listToolAudits({ page: 1, size: 5 });
      setAudits(result.list);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "工具审计加载失败");
    } finally {
      setAuditLoading(false);
    }
  }, [resource]);

  const loadPluginInstallations = useCallback(async () => {
    if (resource !== "plugins") {
      setPluginInstallations([]);
      return;
    }
    try {
      const result = await listPluginInstallations({ page: 1, size: 50 });
      setPluginInstallations(result.list);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "插件安装状态加载失败");
    }
  }, [resource]);

  useEffect(() => {
    void loadItems();
  }, [loadItems]);

  useEffect(() => {
    void loadAudits();
  }, [loadAudits]);

  useEffect(() => {
    void loadPluginInstallations();
  }, [loadPluginInstallations]);

  const pluginInstallationsByCode = useMemo(() => {
    return new Map(pluginInstallations.map((installation) => [installation.pluginCode, installation]));
  }, [pluginInstallations]);

  const columns = useMemo<ColumnDef<CapabilityItemResp>[]>(
    () => [
      {
        header: "名称",
        cell: ({ row }) => (
          <div className="min-w-48">
            <div className="truncate font-medium">{row.original.name}</div>
            <div className="truncate text-xs text-muted-foreground">{row.original.description || "-"}</div>
          </div>
        )
      },
      { accessorKey: "code", header: "编码" },
      {
        header: "类型",
        cell: ({ row }) => <Badge variant="outline">{row.original.kind}</Badge>
      },
      {
        header: "状态",
        cell: ({ row }) => <Badge variant="secondary">{statusLabel(row.original.status)}</Badge>
      },
      {
        header: "配置",
        cell: ({ row }) => <CapabilityMetadata item={row.original} resource={resource} />
      },
      {
        header: "安装",
        cell: ({ row }) => {
          if (resource !== "plugins") {
            return "-";
          }
          const installation = pluginInstallationsByCode.get(row.original.code);
          if (!installation) {
            return <Badge variant="outline">未安装</Badge>;
          }
          return <Badge variant={installation.enabled ? "secondary" : "outline"}>{installation.enabled ? "已启用" : "已停用"}</Badge>;
        }
      },
      {
        header: "风险",
        cell: ({ row }) =>
          row.original.riskLevel ? <Badge variant="outline">{riskLabel(row.original.riskLevel)}</Badge> : "-"
      },
      {
        id: "actions",
        header: "操作",
        cell: ({ row }) => {
          if (resource === "tools") {
            return (
            <PermissionGate permissions={["ai:tool:dryRun"]}>
              <Button
                size="sm"
                variant="outline"
                disabled={runningToolCode === row.original.code}
                onClick={() => void runTool(row.original)}
              >
                <Play />
                试运行
              </Button>
            </PermissionGate>
            );
          }
          if (resource === "plugins") {
            const installation = pluginInstallationsByCode.get(row.original.code);
            return (
              <PermissionGate permissions={["ai:plugin:install"]}>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={installingPluginCode === row.original.code}
                  onClick={() => void enablePlugin(row.original)}
                >
                  <PackageCheck />
                  {installation?.enabled ? "重新启用" : "启用插件"}
                </Button>
              </PermissionGate>
            );
          }
          return null;
        }
      }
    ],
    [installingPluginCode, pluginInstallationsByCode, resource, runningToolCode]
  );

  async function runTool(item: CapabilityItemResp) {
    setRunningToolCode(item.code);
    try {
      const result = await dryRunTool({
        toolCode: item.code,
        input: {
          source: "admin",
          code: item.code
        }
      });
      toast.success(`${item.name} Audit #${result.auditId}`);
      await loadAudits();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "工具试运行失败");
    } finally {
      setRunningToolCode(null);
    }
  }

  async function enablePlugin(item: CapabilityItemResp) {
    setInstallingPluginCode(item.code);
    try {
      const result = await installPlugin({
        pluginCode: item.code,
        version: pluginVersion(item),
        enabled: true,
        permissionGrants: pluginPermissionGrants(item),
        config: {}
      });
      toast.success(`${result.pluginName} 已启用`);
      await loadPluginInstallations();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "插件启用失败");
    } finally {
      setInstallingPluginCode(null);
    }
  }

  return (
    <div className="mx-auto grid w-full max-w-7xl gap-4">
      <section className="flex flex-col gap-3 rounded-lg border bg-background p-4 md:flex-row md:items-center md:justify-between">
        <div className="min-w-0">
          <h1 className="truncate text-base font-semibold">{title}</h1>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span>{total} 条</span>
            <code className="rounded border bg-muted px-1.5 py-0.5">{permission}</code>
          </div>
        </div>
        <Button variant="outline" onClick={() => void loadItems()} disabled={loading}>
          <RefreshCw />
          刷新
        </Button>
      </section>

      <DataTable columns={columns} data={items} loading={loading} emptyText="暂无能力" />

      {resource === "tools" ? (
        <section className="rounded-lg border bg-background p-4">
          <div className="mb-3 flex items-center justify-between gap-3">
            <h2 className="text-sm font-medium">工具调用审计</h2>
            <PermissionGate permissions={["ai:tool:audit:list"]}>
              <Button variant="outline" size="sm" onClick={() => void loadAudits()} disabled={auditLoading}>
                <RefreshCw />
                刷新
              </Button>
            </PermissionGate>
          </div>
          <div className="grid gap-2">
            {audits.map((audit) => (
              <div key={audit.id} className="grid gap-2 rounded-md border p-3 text-sm md:grid-cols-[1fr_auto] md:items-center">
                <div className="min-w-0">
                  <div className="truncate font-medium">{audit.toolCode}</div>
                  <div className="text-xs text-muted-foreground">{audit.createTime}</div>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Badge variant="secondary">{audit.status}</Badge>
                  <Badge variant="outline">{audit.dryRun ? "dry-run" : "live"}</Badge>
                </div>
              </div>
            ))}
            {!audits.length ? <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无审计</div> : null}
          </div>
        </section>
      ) : null}
    </div>
  );
}

function statusLabel(status: number) {
  return status === 1 ? "启用" : "停用";
}

function riskLabel(riskLevel: number) {
  return (
    {
      1: "低",
      2: "中",
      3: "高"
    }[riskLevel] ?? String(riskLevel)
  );
}

function pluginVersion(item: CapabilityItemResp) {
  const version = item.metadata.version;
  return typeof version === "string" && version.trim() ? version.trim() : "0.1.0";
}

function pluginPermissionGrants(item: CapabilityItemResp) {
  const manifest = item.metadata.manifest;
  if (!isRecord(manifest) || !Array.isArray(manifest.permissions)) {
    return [];
  }
  return manifest.permissions.filter((permission): permission is string => typeof permission === "string");
}

function CapabilityMetadata({
  item,
  resource
}: {
  item: CapabilityItemResp;
  resource: CapabilityResource;
}) {
  if (resource === "skills") {
    const routeValues = Object.values(recordValue(item.metadata.modelRoutePolicy))
      .filter((value): value is string => typeof value === "string" && value.trim().length > 0)
      .slice(0, 3);
    const capabilityRefs = arrayValue(item.metadata.capabilityRefs)
      .map((value) => recordValue(value).code)
      .filter((value): value is string => typeof value === "string" && value.trim().length > 0)
      .slice(0, 3);
    return (
      <div className="flex max-w-72 flex-wrap gap-1">
        {[...routeValues, ...capabilityRefs].map((value) => (
          <Badge key={value} variant="outline">
            {value}
          </Badge>
        ))}
        {!routeValues.length && !capabilityRefs.length ? "-" : null}
      </div>
    );
  }
  return "-";
}

function recordValue(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function arrayValue(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
