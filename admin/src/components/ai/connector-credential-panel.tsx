"use client";

import type { ColumnDef } from "@tanstack/react-table";
import { KeyRound, RefreshCw, Save } from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { toast } from "sonner";
import {
  listConnectorCredentials,
  upsertConnectorCredential
} from "@/api/ai/capability";
import { PermissionGate } from "@/components/permission/permission-gate";
import { DataTable } from "@/components/table/data-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
  ConnectorCredentialCommand,
  ConnectorCredentialResp
} from "@/types/ai-capability";

const DEFAULT_CREDENTIAL_FORM: ConnectorCredentialCommand = {
  connectorCode: "github.default",
  scopeType: "tenant",
  scopeId: "1",
  authType: "oauth_app",
  secretRef: "env:GITHUB_CONNECTOR_TOKEN",
  scopes: ["repo"],
  status: 1
};

export function ConnectorCredentialPanel() {
  const [credentials, setCredentials] = useState<ConnectorCredentialResp[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [form, setForm] = useState<ConnectorCredentialCommand>(DEFAULT_CREDENTIAL_FORM);
  const [scopeInput, setScopeInput] = useState("repo");

  const loadCredentials = useCallback(async () => {
    setLoading(true);
    try {
      const result = await listConnectorCredentials({ page: 1, size: 20 });
      setCredentials(result.list);
      setTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "连接器凭据加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadCredentials();
  }, [loadCredentials]);

  const columns = useMemo<ColumnDef<ConnectorCredentialResp>[]>(
    () => [
      {
        header: "连接器",
        cell: ({ row }) => (
          <div className="min-w-44">
            <div className="truncate font-medium">{row.original.connectorCode}</div>
            <div className="truncate text-xs text-muted-foreground">#{row.original.connectorId}</div>
          </div>
        )
      },
      {
        header: "作用域",
        cell: ({ row }) => (
          <div className="flex flex-wrap gap-1.5">
            <Badge variant="outline">{row.original.scopeType}</Badge>
            <Badge variant="secondary">{row.original.scopeId}</Badge>
          </div>
        )
      },
      {
        header: "认证",
        cell: ({ row }) => <Badge variant="outline">{row.original.authType}</Badge>
      },
      {
        header: "Secret",
        cell: ({ row }) => (
          <code className="rounded border bg-muted px-1.5 py-0.5 text-xs">
            {row.original.maskedValue}
          </code>
        )
      },
      {
        header: "Scopes",
        cell: ({ row }) => (
          <div className="flex max-w-64 flex-wrap gap-1.5">
            {row.original.scopes.length ? (
              row.original.scopes.map((scope) => (
                <Badge key={scope} variant="secondary">
                  {scope}
                </Badge>
              ))
            ) : (
              "-"
            )}
          </div>
        )
      },
      {
        header: "状态",
        cell: ({ row }) => <Badge variant="secondary">{row.original.status === 1 ? "启用" : "停用"}</Badge>
      },
      { accessorKey: "createTime", header: "创建时间" }
    ],
    []
  );

  async function submitCredential(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const command: ConnectorCredentialCommand = {
      ...form,
      connectorCode: form.connectorCode.trim(),
      scopeId: form.scopeId.trim(),
      authType: form.authType.trim(),
      secretRef: form.secretRef.trim(),
      scopes: parseScopes(scopeInput),
      status: Number(form.status ?? 1)
    };
    if (!command.connectorCode || !command.scopeId || !command.authType || !command.secretRef) {
      toast.error("请填写连接器凭据");
      return;
    }

    setSubmitting(true);
    try {
      await upsertConnectorCredential(command);
      await loadCredentials();
      toast.success("连接器凭据已保存");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "连接器凭据保存失败");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section className="mx-auto grid w-full max-w-7xl gap-4 rounded-lg border bg-background p-4">
      <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <KeyRound className="size-4 text-primary" />
            <h2 className="truncate text-base font-semibold">连接器凭据</h2>
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span>{total} 条</span>
            <code className="rounded border bg-muted px-1.5 py-0.5">ai:connector:credential:update</code>
          </div>
        </div>
        <Button variant="outline" onClick={() => void loadCredentials()} disabled={loading}>
          <RefreshCw />
          刷新
        </Button>
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_420px]">
        <DataTable
          columns={columns}
          data={credentials}
          loading={loading}
          emptyText="暂无连接器凭据"
          className="shadow-none"
        />

        <PermissionGate permissions={["ai:connector:credential:update"]}>
          <form className="grid self-start rounded-lg border bg-muted/20 p-4" onSubmit={submitCredential}>
            <div className="mb-3 flex items-center justify-between gap-3">
              <h3 className="text-sm font-medium">凭据绑定</h3>
              <Badge variant="outline">env</Badge>
            </div>
            <div className="grid gap-3">
              <Field label="连接器编码">
                <Input
                  value={form.connectorCode}
                  placeholder="github.default"
                  onChange={(event) => setForm({ ...form, connectorCode: event.target.value })}
                />
              </Field>
              <div className="grid gap-3 md:grid-cols-2">
                <Field label="作用域">
                  <Select
                    value={form.scopeType}
                    onValueChange={(scopeType) =>
                      setForm({ ...form, scopeType: scopeType as ConnectorCredentialCommand["scopeType"] })
                    }
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="tenant">tenant</SelectItem>
                      <SelectItem value="user">user</SelectItem>
                      <SelectItem value="app">app</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <Field label="作用域 ID">
                  <Input
                    value={form.scopeId}
                    placeholder="1"
                    onChange={(event) => setForm({ ...form, scopeId: event.target.value })}
                  />
                </Field>
              </div>
              <div className="grid gap-3 md:grid-cols-2">
                <Field label="认证类型">
                  <Input
                    value={form.authType}
                    placeholder="oauth_app"
                    onChange={(event) => setForm({ ...form, authType: event.target.value })}
                  />
                </Field>
                <Field label="状态">
                  <Select
                    value={String(form.status ?? 1)}
                    onValueChange={(status) => setForm({ ...form, status: Number(status) })}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="1">启用</SelectItem>
                      <SelectItem value="0">停用</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
              </div>
              <Field label="Secret Ref">
                <Input
                  value={form.secretRef}
                  placeholder="env:GITHUB_CONNECTOR_TOKEN"
                  onChange={(event) => setForm({ ...form, secretRef: event.target.value })}
                />
              </Field>
              <Field label="Scopes">
                <Input
                  value={scopeInput}
                  placeholder="repo, read:org"
                  onChange={(event) => setScopeInput(event.target.value)}
                />
              </Field>
              <Button type="submit" className="w-fit" disabled={submitting}>
                <Save />
                保存凭据
              </Button>
            </div>
          </form>
        </PermissionGate>
      </div>
    </section>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Label className="grid gap-1.5 text-xs font-medium text-muted-foreground">
      {label}
      {children}
    </Label>
  );
}

function parseScopes(value: string) {
  return value
    .split(/[,\n]/)
    .map((scope) => scope.trim())
    .filter(Boolean);
}
