"use client";

import type { ColumnDef } from "@tanstack/react-table";
import { BrainCircuit, RefreshCw, Save, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { toast } from "sonner";
import { deleteMemory, listMemories, upsertMemory } from "@/api/ai/memory";
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
import type { MemoryCommand, MemoryResp } from "@/types/ai-memory";

const DEFAULT_MEMORY_FORM: MemoryCommand = {
  scopeType: "user",
  scopeId: "1",
  sourceKind: "manual",
  sourceId: "admin-note",
  content: "prefers concise updates",
  summary: "prefers concise updates",
  sensitivity: "preference",
  writePolicy: "user_approved",
  ttlDays: 90,
  metadata: { confirmedByUser: true },
  status: 1
};

export default function AiMemoryPage() {
  const [memories, setMemories] = useState<MemoryResp[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [deletingId, setDeletingId] = useState<number | null>(null);
  const [form, setForm] = useState<MemoryCommand>(DEFAULT_MEMORY_FORM);

  const loadMemories = useCallback(async () => {
    setLoading(true);
    try {
      const result = await listMemories({ page: 1, size: 20 });
      setMemories(result.list);
      setTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Memory 加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadMemories();
  }, [loadMemories]);

  const columns = useMemo<ColumnDef<MemoryResp>[]>(
    () => [
      {
        header: "记忆",
        cell: ({ row }) => (
          <div className="min-w-64">
            <div className="truncate font-medium">{row.original.summary}</div>
            <div className="line-clamp-2 text-xs text-muted-foreground">{row.original.content}</div>
          </div>
        )
      },
      {
        header: "Scope",
        cell: ({ row }) => (
          <div className="flex flex-wrap gap-1.5">
            <Badge variant="outline">{row.original.scopeType}</Badge>
            <Badge variant="secondary">{row.original.scopeId}</Badge>
          </div>
        )
      },
      {
        header: "策略",
        cell: ({ row }) => <Badge variant="outline">{row.original.writePolicy}</Badge>
      },
      {
        header: "敏感度",
        cell: ({ row }) => <Badge variant="secondary">{row.original.sensitivity}</Badge>
      },
      {
        header: "过期",
        cell: ({ row }) => row.original.expiresAt ?? "-"
      },
      {
        id: "actions",
        header: "操作",
        cell: ({ row }) => (
          <PermissionGate permissions={["ai:memory:delete"]}>
            <Button
              size="sm"
              variant="outline"
              disabled={deletingId === row.original.id}
              onClick={() => void removeMemory(row.original.id)}
            >
              <Trash2 />
              删除
            </Button>
          </PermissionGate>
        )
      }
    ],
    [deletingId]
  );

  async function submitMemory(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const content = form.content.trim();
    const command: MemoryCommand = {
      ...form,
      scopeId: form.scopeId.trim(),
      sourceId: form.sourceId?.trim() || "admin-note",
      content,
      summary: content,
      ttlDays: Number(form.ttlDays ?? 90),
      metadata: { confirmedByUser: true },
      status: Number(form.status ?? 1)
    };
    if (!command.scopeId || !command.content) {
      toast.error("请填写 Memory");
      return;
    }

    setSubmitting(true);
    try {
      await upsertMemory(command);
      await loadMemories();
      toast.success("记忆已保存");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Memory 保存失败");
    } finally {
      setSubmitting(false);
    }
  }

  async function removeMemory(id: number) {
    setDeletingId(id);
    try {
      await deleteMemory(id);
      await loadMemories();
      toast.success("记忆已删除");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Memory 删除失败");
    } finally {
      setDeletingId(null);
    }
  }

  return (
    <div className="mx-auto grid w-full max-w-7xl gap-4">
      <section className="flex flex-col gap-3 rounded-lg border bg-background p-4 md:flex-row md:items-center md:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <BrainCircuit className="size-4 text-primary" />
            <h1 className="truncate text-base font-semibold">Memory</h1>
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span>{total} 条</span>
            <code className="rounded border bg-muted px-1.5 py-0.5">ai:memory:list</code>
          </div>
        </div>
        <Button variant="outline" onClick={() => void loadMemories()} disabled={loading}>
          <RefreshCw />
          刷新
        </Button>
      </section>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_420px]">
        <DataTable columns={columns} data={memories} loading={loading} emptyText="暂无记忆" />

        <PermissionGate permissions={["ai:memory:upsert"]}>
          <form className="grid self-start rounded-lg border bg-background p-4" onSubmit={submitMemory}>
            <div className="mb-3 flex items-center justify-between gap-3">
              <h2 className="text-sm font-medium">写入策略</h2>
              <Badge variant="outline">user approved</Badge>
            </div>
            <div className="grid gap-3">
              <div className="grid gap-3 md:grid-cols-2">
                <Field label="Scope">
                  <Select
                    value={form.scopeType}
                    onValueChange={(scopeType) =>
                      setForm({ ...form, scopeType: scopeType as MemoryCommand["scopeType"] })
                    }
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="session">session</SelectItem>
                      <SelectItem value="user">user</SelectItem>
                      <SelectItem value="org">org</SelectItem>
                      <SelectItem value="project">project</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <Field label="Scope ID">
                  <Input
                    value={form.scopeId}
                    placeholder="1"
                    onChange={(event) => setForm({ ...form, scopeId: event.target.value })}
                  />
                </Field>
              </div>
              <Field label="Content">
                <Input
                  value={form.content}
                  placeholder="prefers concise updates"
                  onChange={(event) => setForm({ ...form, content: event.target.value })}
                />
              </Field>
              <div className="grid gap-3 md:grid-cols-2">
                <Field label="Sensitivity">
                  <Select
                    value={form.sensitivity}
                    onValueChange={(sensitivity) =>
                      setForm({ ...form, sensitivity: sensitivity as MemoryCommand["sensitivity"] })
                    }
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="low">low</SelectItem>
                      <SelectItem value="preference">preference</SelectItem>
                      <SelectItem value="confidential">confidential</SelectItem>
                      <SelectItem value="regulated">regulated</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <Field label="TTL Days">
                  <Input
                    type="number"
                    value={form.ttlDays ?? 90}
                    min={1}
                    max={3650}
                    onChange={(event) => setForm({ ...form, ttlDays: Number(event.target.value) })}
                  />
                </Field>
              </div>
              <Button type="submit" className="w-fit" disabled={submitting}>
                <Save />
                保存记忆
              </Button>
            </div>
          </form>
        </PermissionGate>
      </div>
    </div>
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
