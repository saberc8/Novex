"use client";

import type { ColumnDef } from "@tanstack/react-table";
import { Database, FilePlus2, FileText, RefreshCw, Save, Search, Send, Upload, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { toast } from "sonner";
import {
  askDataset,
  createDataset,
  listDatasets,
  listDocuments,
  uploadTextDocument
} from "@/api/ai/knowledge";
import { PermissionGate } from "@/components/permission/permission-gate";
import { DataTable } from "@/components/table/data-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
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
import { Textarea } from "@/components/ui/textarea";
import type { DatasetCommand, DatasetResp, DocumentResp, RagAskResp } from "@/types/ai";

const DEFAULT_DATASET_COMMAND: DatasetCommand = {
  name: "",
  description: "",
  visibility: 1,
  retrievalMode: 3
};

export default function AiKnowledgePage() {
  const [datasets, setDatasets] = useState<DatasetResp[]>([]);
  const [documents, setDocuments] = useState<DocumentResp[]>([]);
  const [selectedDataset, setSelectedDataset] = useState<DatasetResp | null>(null);
  const [keyword, setKeyword] = useState("");
  const [datasetTotal, setDatasetTotal] = useState(0);
  const [documentTotal, setDocumentTotal] = useState(0);
  const [datasetLoading, setDatasetLoading] = useState(false);
  const [documentLoading, setDocumentLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [createSubmitting, setCreateSubmitting] = useState(false);
  const [uploadName, setUploadName] = useState("");
  const [uploadContent, setUploadContent] = useState("");
  const [uploadSubmitting, setUploadSubmitting] = useState(false);
  const [askQuestion, setAskQuestion] = useState("");
  const [askResult, setAskResult] = useState<RagAskResp | null>(null);
  const [askSubmitting, setAskSubmitting] = useState(false);

  const loadDatasets = useCallback(async () => {
    setDatasetLoading(true);
    try {
      const result = await listDatasets({
        page: 1,
        size: 50,
        name: keyword || undefined
      });
      setDatasets(result.list);
      setDatasetTotal(result.total);
      setSelectedDataset((current) => {
        if (!result.list.length) {
          return null;
        }
        if (!current) {
          return result.list[0];
        }
        return result.list.find((dataset) => dataset.id === current.id) ?? result.list[0];
      });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "知识库加载失败");
    } finally {
      setDatasetLoading(false);
    }
  }, [keyword]);

  const loadDocuments = useCallback(async () => {
    if (!selectedDataset) {
      setDocuments([]);
      setDocumentTotal(0);
      return;
    }
    setDocumentLoading(true);
    try {
      const result = await listDocuments(selectedDataset.id, { page: 1, size: 20 });
      setDocuments(result.list);
      setDocumentTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "文档加载失败");
    } finally {
      setDocumentLoading(false);
    }
  }, [selectedDataset]);

  useEffect(() => {
    void loadDatasets();
  }, [loadDatasets]);

  useEffect(() => {
    void loadDocuments();
  }, [loadDocuments]);

  useEffect(() => {
    setAskResult(null);
  }, [selectedDataset?.id]);

  const documentColumns = useMemo<ColumnDef<DocumentResp>[]>(
    () => [
      {
        header: "文档",
        cell: ({ row }) => (
          <div className="flex min-w-48 items-center gap-2">
            <FileText className="size-4 shrink-0 text-muted-foreground" />
            <div className="min-w-0">
              <div className="truncate font-medium">{row.original.name}</div>
              <div className="truncate text-xs text-muted-foreground">{row.original.contentType || "-"}</div>
            </div>
          </div>
        )
      },
      {
        header: "解析",
        cell: ({ row }) => <Badge variant={row.original.parseStatus === 4 ? "destructive" : "secondary"}>{parseStatusLabel(row.original.parseStatus)}</Badge>
      },
      {
        header: "索引",
        cell: ({ row }) => (
          <Badge variant={row.original.ingestionStatus === 5 ? "destructive" : "secondary"}>
            {ingestionStatusLabel(row.original.ingestionStatus)}
          </Badge>
        )
      },
      { accessorKey: "chunkCount", header: "Chunk" },
      { accessorKey: "createUserString", header: "创建人" },
      { accessorKey: "createTime", header: "创建时间" }
    ],
    []
  );

  async function saveDataset(command: DatasetCommand) {
    setCreateSubmitting(true);
    try {
      await createDataset({
        ...command,
        name: command.name.trim(),
        description: command.description.trim()
      });
      setCreateOpen(false);
      await loadDatasets();
      toast.success("知识库已创建");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "知识库创建失败");
    } finally {
      setCreateSubmitting(false);
    }
  }

  async function submitUpload(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedDataset) {
      toast.error("请选择知识库");
      return;
    }
    const name = uploadName.trim();
    const content = uploadContent.trim();
    if (!name || !content) {
      toast.error("请输入文档名称和内容");
      return;
    }
    setUploadSubmitting(true);
    try {
      await uploadTextDocument(selectedDataset.id, {
        name,
        content,
        contentType: "text/plain"
      });
      setUploadName("");
      setUploadContent("");
      await Promise.all([loadDatasets(), loadDocuments()]);
      toast.success("文档已上传");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "文档上传失败");
    } finally {
      setUploadSubmitting(false);
    }
  }

  async function submitAsk(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedDataset) {
      toast.error("请选择知识库");
      return;
    }
    const question = askQuestion.trim();
    if (!question) {
      toast.error("请输入问题");
      return;
    }
    setAskSubmitting(true);
    try {
      setAskResult(await askDataset(selectedDataset.id, { question, limit: 5 }));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "问答失败");
    } finally {
      setAskSubmitting(false);
    }
  }

  return (
    <div
      className="mx-auto grid w-full max-w-7xl items-start gap-4 xl:grid-cols-[360px_1fr]"
      data-testid="knowledge-layout"
    >
      <section className="self-start rounded-lg border bg-background p-4" data-testid="dataset-list-panel">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div className="min-w-0">
            <h2 className="truncate text-base font-semibold">知识库</h2>
            <p className="text-xs text-muted-foreground">{datasetTotal} 个 Dataset</p>
          </div>
          <PermissionGate permissions={["ai:knowledge:create"]}>
            <Button size="sm" onClick={() => setCreateOpen(true)}>
              <FilePlus2 />
              新增知识库
            </Button>
          </PermissionGate>
        </div>

        <div className="mb-3 flex gap-2">
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={keyword}
              className="pl-8"
              placeholder="搜索知识库"
              onChange={(event) => setKeyword(event.target.value)}
            />
          </div>
          <Button variant="outline" size="icon" title="刷新" onClick={() => void loadDatasets()} disabled={datasetLoading}>
            <RefreshCw />
          </Button>
        </div>

        <div className="grid gap-2">
          {datasetLoading ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">加载中</div>
          ) : null}
          {!datasetLoading && !datasets.length ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无知识库</div>
          ) : null}
          {datasets.map((dataset) => (
            <button
              key={dataset.id}
              type="button"
              aria-pressed={selectedDataset?.id === dataset.id}
              className={`rounded-md border p-3 text-left text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${selectedDataset?.id === dataset.id ? "border-primary bg-primary/5" : "bg-background hover:bg-muted/35"}`}
              data-testid={`dataset-card-${dataset.id}`}
              onClick={() => setSelectedDataset(dataset)}
            >
              <div className="mb-2 flex min-w-0 items-center gap-2">
                <Database className="size-4 shrink-0 text-muted-foreground" />
                <span className="truncate font-medium">{dataset.name}</span>
              </div>
              {dataset.description ? (
                <div className="mb-2 line-clamp-2 text-xs text-muted-foreground">{dataset.description}</div>
              ) : null}
              <div className="mb-2 flex flex-wrap gap-1.5">
                <Badge variant="secondary">{datasetStatusLabel(dataset.status)}</Badge>
                <Badge variant="outline">{visibilityLabel(dataset.visibility)}</Badge>
                <Badge variant="outline">{retrievalModeLabel(dataset.retrievalMode)}</Badge>
              </div>
              <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground">
                <span>{dataset.documentCount} 文档</span>
                <span>{dataset.chunkCount} Chunk</span>
              </div>
            </button>
          ))}
        </div>
      </section>

      <section className="grid self-start content-start gap-4" data-testid="documents-panel">
        <div className="flex flex-col gap-3 rounded-lg border bg-background p-4 md:flex-row md:items-center md:justify-between">
          <div className="min-w-0">
            <div className="truncate text-sm font-medium">{selectedDataset?.name ?? "文档"}</div>
            <p className="text-xs text-muted-foreground">{documentTotal} 个 Document</p>
          </div>
          <Button
            variant="outline"
            onClick={() => void loadDocuments()}
            disabled={!selectedDataset || documentLoading}
          >
            <RefreshCw />
            刷新
          </Button>
        </div>
        <div className="grid gap-4 lg:grid-cols-2">
          <PermissionGate permissions={["ai:knowledge:document:create"]}>
            <form className="grid gap-3 rounded-lg border bg-background p-4" onSubmit={submitUpload}>
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-sm font-medium">文本上传</h3>
                <Badge variant="outline">text/plain</Badge>
              </div>
              <Field label="文档名称">
                <Input
                  value={uploadName}
                  placeholder="文档名称"
                  onChange={(event) => setUploadName(event.target.value)}
                />
              </Field>
              <Field label="内容">
                <Textarea
                  value={uploadContent}
                  className="min-h-32"
                  placeholder="文本或 Markdown"
                  onChange={(event) => setUploadContent(event.target.value)}
                />
              </Field>
              <Button type="submit" className="w-fit" disabled={!selectedDataset || uploadSubmitting}>
                <Upload />
                上传文档
              </Button>
            </form>
          </PermissionGate>

          <PermissionGate permissions={["ai:knowledge:ask"]}>
            <form className="grid gap-3 rounded-lg border bg-background p-4" onSubmit={submitAsk}>
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-sm font-medium">检索问答</h3>
                {askResult ? <Badge variant="outline">Trace #{askResult.traceId}</Badge> : null}
              </div>
              <Field label="问题">
                <Input
                  value={askQuestion}
                  placeholder="输入测试问题"
                  onChange={(event) => setAskQuestion(event.target.value)}
                />
              </Field>
              <Button type="submit" className="w-fit" disabled={!selectedDataset || askSubmitting}>
                <Send />
                提问
              </Button>
              {askResult ? (
                <div className="grid gap-3 rounded-md border bg-muted/20 p-3 text-sm">
                  <div className="whitespace-pre-wrap leading-6">{askResult.answer}</div>
                  {askResult.citations.length ? (
                    <div className="flex flex-wrap gap-2">
                      {askResult.citations.map((citation) => (
                        <Badge key={`${citation.documentId}-${citation.chunkId}`} variant="secondary">
                          {citation.chunkId}
                        </Badge>
                      ))}
                    </div>
                  ) : null}
                </div>
              ) : null}
            </form>
          </PermissionGate>
        </div>
        <DataTable
          columns={documentColumns}
          data={documents}
          loading={documentLoading}
          emptyText={selectedDataset ? "暂无文档" : "请选择知识库"}
        />
      </section>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>新增知识库</DialogTitle>
            <DialogDescription>创建 Dataset 元数据</DialogDescription>
          </DialogHeader>
          <DatasetForm
            submitting={createSubmitting}
            onSubmit={saveDataset}
            onCancel={() => setCreateOpen(false)}
          />
        </DialogContent>
      </Dialog>
    </div>
  );
}

function DatasetForm({
  submitting,
  onSubmit,
  onCancel
}: {
  submitting?: boolean;
  onSubmit: (command: DatasetCommand) => void;
  onCancel: () => void;
}) {
  const [form, setForm] = useState<DatasetCommand>(DEFAULT_DATASET_COMMAND);

  useEffect(() => {
    setForm(DEFAULT_DATASET_COMMAND);
  }, []);

  return (
    <form
      className="grid gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(form);
      }}
    >
      <Field label="名称">
        <Input
          value={form.name}
          placeholder="知识库名称"
          onChange={(event) => setForm({ ...form, name: event.target.value })}
          required
        />
      </Field>
      <Field label="描述">
        <Textarea
          value={form.description}
          placeholder="描述这个知识库的内容范围"
          onChange={(event) => setForm({ ...form, description: event.target.value })}
        />
      </Field>
      <div className="grid gap-3 md:grid-cols-2">
        <Field label="可见性">
          <Select
            value={String(form.visibility)}
            onValueChange={(visibility) => setForm({ ...form, visibility: Number(visibility) })}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="1">私有</SelectItem>
              <SelectItem value="2">租户</SelectItem>
              <SelectItem value="3">公开</SelectItem>
            </SelectContent>
          </Select>
        </Field>
        <Field label="检索模式">
          <Select
            value={String(form.retrievalMode)}
            onValueChange={(retrievalMode) => setForm({ ...form, retrievalMode: Number(retrievalMode) })}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="1">向量</SelectItem>
              <SelectItem value="2">关键词</SelectItem>
              <SelectItem value="3">混合</SelectItem>
            </SelectContent>
          </Select>
        </Field>
      </div>
      <div className="flex justify-end gap-2">
        <Button type="button" variant="outline" onClick={onCancel}>
          <X />
          取消
        </Button>
        <Button type="submit" disabled={submitting}>
          <Save />
          保存
        </Button>
      </div>
    </form>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid gap-2">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

function datasetStatusLabel(status: number) {
  return labelOf(status, {
    1: "草稿",
    2: "已发布",
    3: "已归档"
  });
}

function visibilityLabel(visibility: number) {
  return labelOf(visibility, {
    1: "私有",
    2: "租户",
    3: "公开"
  });
}

function retrievalModeLabel(mode: number) {
  return labelOf(mode, {
    1: "向量",
    2: "关键词",
    3: "混合"
  });
}

function parseStatusLabel(status: number) {
  return labelOf(status, {
    1: "待解析",
    2: "解析中",
    3: "已解析",
    4: "失败"
  });
}

function ingestionStatusLabel(status: number) {
  return labelOf(status, {
    1: "待索引",
    2: "切片中",
    3: "向量化",
    4: "已索引",
    5: "失败"
  });
}

function labelOf(value: number, labels: Record<number, string>) {
  return labels[value] ?? String(value);
}
