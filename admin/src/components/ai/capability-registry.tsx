"use client";

import type { ColumnDef } from "@tanstack/react-table";
import { Play, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  dryRunTool,
  listConnectors,
  listMcpServers,
  listPlugins,
  listToolAudits,
  listTools,
  listTriggers
} from "@/api/ai/capability";
import { PermissionGate } from "@/components/permission/permission-gate";
import { DataTable } from "@/components/table/data-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { PageResult } from "@/types/api";
import type { CapabilityItemResp, ToolCallAuditResp } from "@/types/ai-capability";

type CapabilityResource = "tools" | "connectors" | "plugins" | "triggers" | "mcpServers";

interface CapabilityRegistryProps {
  title: string;
  resource: CapabilityResource;
  permission: string;
}

const LOADERS: Record<
  CapabilityResource,
  () => Promise<PageResult<CapabilityItemResp>>
> = {
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
  const [loading, setLoading] = useState(false);
  const [auditLoading, setAuditLoading] = useState(false);
  const [runningToolCode, setRunningToolCode] = useState<string | null>(null);

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

  useEffect(() => {
    void loadItems();
  }, [loadItems]);

  useEffect(() => {
    void loadAudits();
  }, [loadAudits]);

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
        header: "风险",
        cell: ({ row }) =>
          row.original.riskLevel ? <Badge variant="outline">{riskLabel(row.original.riskLevel)}</Badge> : "-"
      },
      {
        id: "actions",
        header: "操作",
        cell: ({ row }) =>
          resource === "tools" ? (
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
          ) : null
      }
    ],
    [resource, runningToolCode]
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
