"use client";

import type { ColumnDef } from "@tanstack/react-table";
import { Bot, FileArchive, FileUp, PackageCheck, Play, RefreshCw } from "lucide-react";
import type { RefObject } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import {
  dryRunTool,
  importSkill,
  importSkillFromSource,
  importSkillPackage,
  installPlugin,
  listConnectors,
  listMcpServers,
  listPluginInstallations,
  listPlugins,
  listSkills,
  listToolAudits,
  listTools,
  listTriggers,
  previewSkillImport
} from "@/api/ai/capability";
import { PermissionGate } from "@/components/permission/permission-gate";
import { DataTable } from "@/components/table/data-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle
} from "@/components/ui/sheet";
import { Textarea } from "@/components/ui/textarea";
import type { PageResult } from "@/types/api";
import type {
  CapabilityItemResp,
  PluginInstallationResp,
  SkillImportPreviewResp,
  SkillImportPreviewItemResp,
  ToolCallAuditResp
} from "@/types/ai-capability";

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
  const [importingSkill, setImportingSkill] = useState(false);
  const [aiImportOpen, setAiImportOpen] = useState(false);
  const [skillImportSource, setSkillImportSource] = useState("");
  const [skillImportPreview, setSkillImportPreview] = useState<SkillImportPreviewResp | null>(null);
  const [skillImportPreviewing, setSkillImportPreviewing] = useState(false);
  const [installingSkillPath, setInstallingSkillPath] = useState<string | null>(null);
  const skillFileInputRef = useRef<HTMLInputElement>(null);
  const skillPackageInputRef = useRef<HTMLInputElement>(null);

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

  async function handleSkillImportFile(file: File) {
    const formData = new FormData();
    formData.append("file", file);
    setImportingSkill(true);
    try {
      const result = await importSkill(formData);
      toast.success(`${result.name} 已导入`);
      await loadItems();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Skill 导入失败");
    } finally {
      setImportingSkill(false);
    }
  }

  async function handleSkillPackageImport(file: File) {
    const formData = new FormData();
    formData.append("file", file);
    setImportingSkill(true);
    try {
      const result = await importSkillPackage(formData);
      toast.success(`${result.skill.name} 已导入，references ${result.referenceCount}`);
      setAiImportOpen(false);
      setSkillImportPreview(null);
      await loadItems();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Skill 压缩包导入失败");
    } finally {
      setImportingSkill(false);
    }
  }

  async function handleSkillImportPreview() {
    const source = skillImportSource.trim();
    if (!source) {
      toast.error("请先输入导入需求或 GitHub 地址");
      return;
    }
    setSkillImportPreviewing(true);
    try {
      const result = await previewSkillImport({ source });
      setSkillImportPreview(result);
      if (!result.skills.length) {
        toast.error("未发现可导入的 Skill");
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Skill 来源分析失败");
    } finally {
      setSkillImportPreviewing(false);
    }
  }

  async function handleSourceSkillInstall(item: SkillImportPreviewItemResp) {
    const source = skillImportSource.trim();
    if (!source) {
      toast.error("请先输入导入需求或 GitHub 地址");
      return;
    }
    setInstallingSkillPath(item.path);
    try {
      const result = await importSkillFromSource({
        source,
        skillPath: item.path
      });
      toast.success(`${result.skill.name} 已安装，references ${result.referenceCount}`);
      setAiImportOpen(false);
      setSkillImportPreview(null);
      await loadItems();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Skill 安装失败");
    } finally {
      setInstallingSkillPath(null);
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
        <div className="flex flex-wrap gap-2">
          {resource === "skills" ? (
            <PermissionGate permissions={["ai:skill:import"]}>
              <Button
                variant="outline"
                onClick={() => skillFileInputRef.current?.click()}
                disabled={importingSkill}
              >
                <FileUp />
                导入 Skill
              </Button>
              <input
                ref={skillFileInputRef}
                className="hidden"
                type="file"
                accept=".md,.json"
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (file) void handleSkillImportFile(file);
                  event.target.value = "";
                }}
              />
            </PermissionGate>
          ) : null}
          <Button variant="outline" onClick={() => void loadItems()} disabled={loading}>
            <RefreshCw />
            刷新
          </Button>
        </div>
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

      {resource === "skills" ? (
        <PermissionGate permissions={["ai:skill:import"]}>
          <Button
            className="fixed bottom-5 right-5 z-40 shadow-lg"
            onClick={() => setAiImportOpen(true)}
          >
            <Bot />
            AI 导入 Skills
          </Button>
          <SkillImportAssistantSheet
            open={aiImportOpen}
            source={skillImportSource}
            preview={skillImportPreview}
            previewing={skillImportPreviewing}
            importing={importingSkill}
            installingPath={installingSkillPath}
            packageInputRef={skillPackageInputRef}
            onOpenChange={setAiImportOpen}
            onSourceChange={setSkillImportSource}
            onPreview={() => void handleSkillImportPreview()}
            onInstall={(item) => void handleSourceSkillInstall(item)}
            onPackageSelect={(file) => void handleSkillPackageImport(file)}
          />
        </PermissionGate>
      ) : null}
    </div>
  );
}

function SkillImportAssistantSheet({
  open,
  source,
  preview,
  previewing,
  importing,
  installingPath,
  packageInputRef,
  onOpenChange,
  onSourceChange,
  onPreview,
  onInstall,
  onPackageSelect
}: {
  open: boolean;
  source: string;
  preview: SkillImportPreviewResp | null;
  previewing: boolean;
  importing: boolean;
  installingPath: string | null;
  packageInputRef: RefObject<HTMLInputElement | null>;
  onOpenChange: (open: boolean) => void;
  onSourceChange: (value: string) => void;
  onPreview: () => void;
  onInstall: (item: SkillImportPreviewItemResp) => void;
  onPackageSelect: (file: File) => void;
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="flex w-full flex-col gap-4 sm:max-w-xl">
        <SheetHeader>
          <SheetTitle>AI 导入 Skills</SheetTitle>
          <SheetDescription>
            粘贴 GitHub Skill 地址或上传 zip 包，references 会保存并在对话中检索注入，scripts 仅保存不执行。
          </SheetDescription>
        </SheetHeader>

        <div className="grid gap-2">
          <Label htmlFor="skill-import-source">导入需求或 GitHub 地址</Label>
          <Textarea
            id="skill-import-source"
            value={source}
            onChange={(event) => onSourceChange(event.target.value)}
            className="min-h-24"
            placeholder="https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer"
          />
        </div>

        <div className="flex flex-wrap gap-2">
          <Button onClick={onPreview} disabled={previewing || importing}>
            <Bot />
            {previewing ? "分析中" : "分析"}
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={importing}
            onClick={() => packageInputRef.current?.click()}
          >
            <FileArchive />
            上传 zip
          </Button>
          <input
            ref={packageInputRef}
            className="hidden"
            type="file"
            accept=".zip,application/zip"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) onPackageSelect(file);
              event.target.value = "";
            }}
          />
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {preview ? (
            <div className="grid gap-3">
              {preview.warnings.map((warning) => (
                <div key={warning} className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-900">
                  {warning}
                </div>
              ))}
              {preview.skills.map((item) => (
                <div key={item.path} className="grid gap-3 rounded-md border p-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">{item.name}</div>
                    <div className="truncate text-xs text-muted-foreground">{item.description || item.path}</div>
                  </div>
                  <div className="flex flex-wrap gap-1">
                    <Badge variant="outline">{item.path}</Badge>
                    <Badge variant="secondary">references {item.referenceCount}</Badge>
                    <Badge variant="outline">scripts {item.scriptCount}</Badge>
                    <Badge variant="outline">assets {item.assetCount}</Badge>
                  </div>
                  <div className="flex justify-end">
                    <Button
                      size="sm"
                      onClick={() => onInstall(item)}
                      disabled={Boolean(installingPath) || importing}
                    >
                      {installingPath === item.path ? "安装中" : "安装"}
                    </Button>
                  </div>
                </div>
              ))}
              {!preview.skills.length ? (
                <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
                  未发现可导入的 Skill
                </div>
              ) : null}
            </div>
          ) : (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
              分析后会显示可安装的 Skill、references、scripts 和 assets 数量
            </div>
          )}
        </div>
      </SheetContent>
    </Sheet>
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
