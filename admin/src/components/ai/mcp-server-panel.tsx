"use client";

import { Network, Save } from "lucide-react";
import { useState, type FormEvent, type ReactNode } from "react";
import { toast } from "sonner";
import { upsertMcpServer } from "@/api/ai/capability";
import { PermissionGate } from "@/components/permission/permission-gate";
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
import type { McpServerCommand } from "@/types/ai-capability";

const DEFAULT_MCP_FORM: McpServerCommand = {
  code: "docs.search",
  name: "Docs Search",
  endpointUrl: "https://mcp.example.com/sse",
  transportKind: "streamable_http",
  authScope: "tenant",
  authType: "bearer_env",
  secretRef: "env:DOCS_MCP_TOKEN",
  networkAllowlist: ["mcp.example.com"],
  toolAllowlist: ["docs.search", "docs.read"],
  discoveredTools: [],
  enabled: true
};

export function McpServerPanel({ onSaved }: { onSaved?: () => void }) {
  const [form, setForm] = useState<McpServerCommand>(DEFAULT_MCP_FORM);
  const [networkInput, setNetworkInput] = useState("mcp.example.com");
  const [toolInput, setToolInput] = useState("docs.search, docs.read");
  const [submitting, setSubmitting] = useState(false);

  async function submitMcpServer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const command: McpServerCommand = {
      ...form,
      code: form.code.trim(),
      name: form.name.trim(),
      endpointUrl: form.endpointUrl?.trim() || null,
      secretRef: form.secretRef?.trim() || null,
      networkAllowlist: parseList(networkInput),
      toolAllowlist: parseList(toolInput),
      discoveredTools: [],
      enabled: form.enabled ?? true
    };
    if (!command.code || !command.name) {
      toast.error("请填写 MCP Server");
      return;
    }

    setSubmitting(true);
    try {
      await upsertMcpServer(command);
      toast.success("MCP Server 已保存");
      onSaved?.();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "MCP Server 保存失败");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section className="mx-auto grid w-full max-w-7xl gap-4 rounded-lg border bg-background p-4">
      <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Network className="size-4 text-primary" />
            <h2 className="truncate text-base font-semibold">MCP 注册</h2>
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <code className="rounded border bg-muted px-1.5 py-0.5">ai:mcp:update</code>
            <Badge variant="outline">allow-list</Badge>
          </div>
        </div>
      </div>

      <PermissionGate permissions={["ai:mcp:update"]}>
        <form className="grid gap-3" onSubmit={submitMcpServer}>
          <div className="grid gap-3 md:grid-cols-2">
            <Field label="Server 编码">
              <Input
                value={form.code}
                placeholder="docs.search"
                onChange={(event) => setForm({ ...form, code: event.target.value })}
              />
            </Field>
            <Field label="名称">
              <Input
                value={form.name}
                placeholder="Docs Search"
                onChange={(event) => setForm({ ...form, name: event.target.value })}
              />
            </Field>
          </div>
          <Field label="Endpoint URL">
            <Input
              value={form.endpointUrl ?? ""}
              placeholder="https://mcp.example.com/sse"
              onChange={(event) => setForm({ ...form, endpointUrl: event.target.value })}
            />
          </Field>
          <div className="grid gap-3 md:grid-cols-4">
            <Field label="Transport">
              <Select
                value={form.transportKind}
                onValueChange={(transportKind) =>
                  setForm({ ...form, transportKind: transportKind as McpServerCommand["transportKind"] })
                }
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="streamable_http">streamable_http</SelectItem>
                  <SelectItem value="sse">sse</SelectItem>
                  <SelectItem value="stdio">stdio</SelectItem>
                  <SelectItem value="builtin">builtin</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Field label="Auth Scope">
              <Select
                value={form.authScope}
                onValueChange={(authScope) =>
                  setForm({ ...form, authScope: authScope as McpServerCommand["authScope"] })
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
            <Field label="Auth Type">
              <Select
                value={form.authType}
                onValueChange={(authType) =>
                  setForm({ ...form, authType: authType as McpServerCommand["authType"] })
                }
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">none</SelectItem>
                  <SelectItem value="bearer_env">bearer_env</SelectItem>
                  <SelectItem value="oauth">oauth</SelectItem>
                  <SelectItem value="headers">headers</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Field label="状态">
              <Select
                value={form.enabled === false ? "false" : "true"}
                onValueChange={(enabled) => setForm({ ...form, enabled: enabled === "true" })}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="true">启用</SelectItem>
                  <SelectItem value="false">停用</SelectItem>
                </SelectContent>
              </Select>
            </Field>
          </div>
          <Field label="Secret Ref">
            <Input
              value={form.secretRef ?? ""}
              placeholder="env:DOCS_MCP_TOKEN"
              onChange={(event) => setForm({ ...form, secretRef: event.target.value })}
            />
          </Field>
          <div className="grid gap-3 md:grid-cols-2">
            <Field label="Network Allow-list">
              <Input
                value={networkInput}
                placeholder="mcp.example.com"
                onChange={(event) => setNetworkInput(event.target.value)}
              />
            </Field>
            <Field label="Tool Allow-list">
              <Input
                value={toolInput}
                placeholder="docs.search, docs.read"
                onChange={(event) => setToolInput(event.target.value)}
              />
            </Field>
          </div>
          <Button type="submit" className="w-fit" disabled={submitting}>
            <Save />
            保存 MCP Server
          </Button>
        </form>
      </PermissionGate>
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

function parseList(value: string) {
  return value
    .split(/[,\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
}
