"use client";

import type { ColumnDef } from "@tanstack/react-table";
import { Ban, KeyRound, Link2, Plus, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { toast } from "sonner";
import {
  createApiKey,
  createPublicLink,
  listApiKeys,
  listPublicLinks,
  revokeApiKey,
  revokePublicLink
} from "@/api/ai/integration";
import { PermissionGate } from "@/components/permission/permission-gate";
import { DataTable } from "@/components/table/data-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type {
  ApiKeyCommand,
  ApiKeyResp,
  PublicLinkCommand,
  PublicLinkResp
} from "@/types/ai-integration";

const DEFAULT_API_KEY: ApiKeyCommand = {
  appId: "training_app",
  name: "Training API",
  permissionScope: ["app:training:ask"],
  qpsLimit: 5,
  quotaLimit: 1000,
  expiresAt: "2026-12-31T00:00:00Z"
};

const DEFAULT_PUBLIC_LINK: PublicLinkCommand = {
  appId: "training_app",
  name: "Training Preview",
  path: "/ask",
  permissionScope: ["app:training:ask"],
  qpsLimit: 2,
  quotaLimit: 200,
  expiresAt: "2026-12-31T00:00:00Z"
};

export default function AiIntegrationsPage() {
  const [apiKeys, setApiKeys] = useState<ApiKeyResp[]>([]);
  const [publicLinks, setPublicLinks] = useState<PublicLinkResp[]>([]);
  const [apiKeyTotal, setApiKeyTotal] = useState(0);
  const [publicLinkTotal, setPublicLinkTotal] = useState(0);
  const [apiKeyForm, setApiKeyForm] = useState<ApiKeyCommand>(DEFAULT_API_KEY);
  const [publicLinkForm, setPublicLinkForm] = useState<PublicLinkCommand>(DEFAULT_PUBLIC_LINK);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState<"api-key" | "public-link" | null>(null);
  const [revokingId, setRevokingId] = useState<number | null>(null);
  const [plainKey, setPlainKey] = useState<string | null>(null);
  const [generatedPublicUrl, setGeneratedPublicUrl] = useState<string | null>(null);

  const loadIntegrations = useCallback(async () => {
    setLoading(true);
    try {
      const [keys, links] = await Promise.all([
        listApiKeys({ page: 1, size: 20 }),
        listPublicLinks({ page: 1, size: 20 })
      ]);
      setApiKeys(keys.list);
      setApiKeyTotal(keys.total);
      setPublicLinks(links.list);
      setPublicLinkTotal(links.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "集成入口加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadIntegrations();
  }, [loadIntegrations]);

  const apiKeyColumns = useMemo<ColumnDef<ApiKeyResp>[]>(
    () => [
      {
        header: "API Key",
        cell: ({ row }) => (
          <div className="min-w-56">
            <div className="truncate font-medium">{row.original.name}</div>
            <div className="font-mono text-xs text-muted-foreground">{row.original.maskedKey}</div>
          </div>
        )
      },
      {
        header: "App",
        cell: ({ row }) => <Badge variant="outline">{row.original.appId}</Badge>
      },
      {
        header: "Limit",
        cell: ({ row }) => (
          <UsageSummary
            qpsUsed={row.original.usageSummary.qpsUsed}
            qpsLimit={row.original.qpsLimit}
            quotaUsed={row.original.usageSummary.quotaUsed}
            quotaLimit={row.original.quotaLimit}
          />
        )
      },
      {
        header: "Scope",
        cell: ({ row }) => <ScopeList items={row.original.permissionScope} />
      },
      {
        id: "actions",
        header: "操作",
        cell: ({ row }) => (
          <PermissionGate permissions={["ai:integration:revoke"]}>
            <Button
              size="sm"
              variant="outline"
              disabled={revokingId === row.original.id}
              onClick={() => void revokeKey(row.original.id)}
            >
              <Ban />
              撤销 API Key
            </Button>
          </PermissionGate>
        )
      }
    ],
    [revokingId]
  );

  const publicLinkColumns = useMemo<ColumnDef<PublicLinkResp>[]>(
    () => [
      {
        header: "Public Link",
        cell: ({ row }) => (
          <div className="min-w-64">
            <div className="truncate font-medium">{row.original.name}</div>
            <div className="truncate font-mono text-xs text-muted-foreground">{row.original.publicUrl}</div>
          </div>
        )
      },
      {
        header: "Path",
        cell: ({ row }) => <Badge variant="outline">{row.original.path}</Badge>
      },
      {
        header: "Limit",
        cell: ({ row }) => (
          <UsageSummary
            qpsUsed={row.original.usageSummary.qpsUsed}
            qpsLimit={row.original.qpsLimit}
            quotaUsed={row.original.usageSummary.quotaUsed}
            quotaLimit={row.original.quotaLimit}
          />
        )
      },
      {
        header: "Scope",
        cell: ({ row }) => <ScopeList items={row.original.permissionScope} />
      },
      {
        id: "actions",
        header: "操作",
        cell: ({ row }) => (
          <PermissionGate permissions={["ai:integration:revoke"]}>
            <Button
              size="sm"
              variant="outline"
              disabled={revokingId === row.original.id}
              onClick={() => void revokeLink(row.original.id)}
            >
              <Ban />
              撤销 Public Link
            </Button>
          </PermissionGate>
        )
      }
    ],
    [revokingId]
  );

  async function submitApiKey() {
    setSubmitting("api-key");
    try {
      const result = await createApiKey(apiKeyForm);
      setPlainKey(result.plainKey);
      await loadIntegrations();
      toast.success("API Key 已创建");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "API Key 创建失败");
    } finally {
      setSubmitting(null);
    }
  }

  async function submitPublicLink() {
    setSubmitting("public-link");
    try {
      const result = await createPublicLink(publicLinkForm);
      setGeneratedPublicUrl(result.publicUrl);
      await loadIntegrations();
      toast.success("Public Link 已创建");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Public Link 创建失败");
    } finally {
      setSubmitting(null);
    }
  }

  async function revokeKey(id: number) {
    setRevokingId(id);
    try {
      await revokeApiKey(id);
      await loadIntegrations();
      toast.success("API Key 已撤销");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "API Key 撤销失败");
    } finally {
      setRevokingId(null);
    }
  }

  async function revokeLink(id: number) {
    setRevokingId(id);
    try {
      await revokePublicLink(id);
      await loadIntegrations();
      toast.success("Public Link 已撤销");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Public Link 撤销失败");
    } finally {
      setRevokingId(null);
    }
  }

  return (
    <div className="mx-auto grid w-full max-w-7xl gap-4">
      <section className="flex flex-col gap-3 rounded-lg border bg-background p-4 md:flex-row md:items-center md:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <KeyRound className="size-4 text-primary" />
            <h1 className="truncate text-base font-semibold">Integration Entry</h1>
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span>{apiKeyTotal} Keys</span>
            <span>{publicLinkTotal} Links</span>
            <code className="rounded border bg-muted px-1.5 py-0.5">ai:integration:list</code>
          </div>
        </div>
        <Button variant="outline" onClick={() => void loadIntegrations()} disabled={loading}>
          <RefreshCw />
          刷新
        </Button>
      </section>

      <div className="grid gap-4 xl:grid-cols-2">
        <PermissionGate permissions={["ai:integration:create"]}>
          <section className="grid gap-3 rounded-lg border bg-background p-4">
            <div className="flex items-center gap-2">
              <KeyRound className="size-4 text-primary" />
              <h2 className="text-sm font-medium">API Key</h2>
            </div>
            <div className="grid gap-3 md:grid-cols-2">
              <Field label="App">
                <Input
                  value={apiKeyForm.appId}
                  onChange={(event) => setApiKeyForm({ ...apiKeyForm, appId: event.target.value })}
                />
              </Field>
              <Field label="Name">
                <Input
                  value={apiKeyForm.name}
                  onChange={(event) => setApiKeyForm({ ...apiKeyForm, name: event.target.value })}
                />
              </Field>
            </div>
            <LimitFields
              qps={apiKeyForm.qpsLimit}
              quota={apiKeyForm.quotaLimit}
              onQps={(qpsLimit) => setApiKeyForm({ ...apiKeyForm, qpsLimit })}
              onQuota={(quotaLimit) => setApiKeyForm({ ...apiKeyForm, quotaLimit })}
            />
            <Button className="w-fit" disabled={submitting === "api-key"} onClick={() => void submitApiKey()}>
              <Plus />
              创建 API Key
            </Button>
            {plainKey ? (
              <div className="rounded-md border bg-muted p-3">
                <div className="mb-1 text-xs text-muted-foreground">Plain Key</div>
                <code className="break-all text-xs">{plainKey}</code>
              </div>
            ) : null}
          </section>
        </PermissionGate>

        <PermissionGate permissions={["ai:integration:create"]}>
          <section className="grid gap-3 rounded-lg border bg-background p-4">
            <div className="flex items-center gap-2">
              <Link2 className="size-4 text-primary" />
              <h2 className="text-sm font-medium">Public Link</h2>
            </div>
            <div className="grid gap-3 md:grid-cols-2">
              <Field label="App">
                <Input
                  value={publicLinkForm.appId}
                  onChange={(event) =>
                    setPublicLinkForm({ ...publicLinkForm, appId: event.target.value })
                  }
                />
              </Field>
              <Field label="Path">
                <Input
                  value={publicLinkForm.path}
                  onChange={(event) => setPublicLinkForm({ ...publicLinkForm, path: event.target.value })}
                />
              </Field>
            </div>
            <Field label="Name">
              <Input
                value={publicLinkForm.name}
                onChange={(event) => setPublicLinkForm({ ...publicLinkForm, name: event.target.value })}
              />
            </Field>
            <LimitFields
              qps={publicLinkForm.qpsLimit}
              quota={publicLinkForm.quotaLimit}
              onQps={(qpsLimit) => setPublicLinkForm({ ...publicLinkForm, qpsLimit })}
              onQuota={(quotaLimit) => setPublicLinkForm({ ...publicLinkForm, quotaLimit })}
            />
            <Button className="w-fit" disabled={submitting === "public-link"} onClick={() => void submitPublicLink()}>
              <Plus />
              创建 Public Link
            </Button>
            {generatedPublicUrl ? (
              <div className="rounded-md border bg-muted p-3">
                <div className="mb-1 flex items-center gap-2 text-xs text-muted-foreground">
                  <Link2 className="size-3.5" />
                  Generated Public URL
                </div>
                <code className="break-all text-xs">{generatedPublicUrl}</code>
              </div>
            ) : null}
          </section>
        </PermissionGate>
      </div>

      <div className="grid gap-4 xl:grid-cols-2">
        <DataTable columns={apiKeyColumns} data={apiKeys} loading={loading} emptyText="暂无 API Key" />
        <DataTable columns={publicLinkColumns} data={publicLinks} loading={loading} emptyText="暂无 Public Link" />
      </div>
    </div>
  );
}

function LimitFields({
  qps,
  quota,
  onQps,
  onQuota
}: {
  qps: number;
  quota: number;
  onQps: (value: number) => void;
  onQuota: (value: number) => void;
}) {
  return (
    <div className="grid gap-3 md:grid-cols-2">
      <Field label="QPS">
        <Input type="number" min={1} value={qps} onChange={(event) => onQps(Number(event.target.value))} />
      </Field>
      <Field label="Quota">
        <Input
          type="number"
          min={1}
          value={quota}
          onChange={(event) => onQuota(Number(event.target.value))}
        />
      </Field>
    </div>
  );
}

function ScopeList({ items }: { items: string[] }) {
  return (
    <div className="flex flex-wrap gap-1">
      {items.map((item) => (
        <Badge key={item} variant="secondary">
          {item}
        </Badge>
      ))}
    </div>
  );
}

function UsageSummary({
  qpsUsed,
  qpsLimit,
  quotaUsed,
  quotaLimit
}: {
  qpsUsed: number;
  qpsLimit: number;
  quotaUsed: number;
  quotaLimit: number;
}) {
  return (
    <div className="grid gap-1 text-xs text-muted-foreground">
      <span>{qpsUsed} / {qpsLimit} QPS</span>
      <span>{quotaUsed} / {quotaLimit} quota</span>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid gap-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      {children}
    </div>
  );
}
