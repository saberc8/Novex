"use client";

import { useEffect, useMemo, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Blocks,
  ChevronDown,
  Circle,
  Clock3,
  FileText,
  Folder,
  FolderGit2,
  Globe2,
  MessageCircle,
  PanelLeft,
  Plus,
  Search,
  Settings,
  ShieldAlert,
  SquarePen,
  Wrench
} from "lucide-react";
import { createConfiguredModelAgentRun, listAgentRunEvents } from "@/api/agent";
import { listMcpServers, listMcpTools, listSkills } from "@/api/capability";
import { uploadKnowledgeFile } from "@/api/knowledge";
import { ensureWorkbenchDataset } from "@/api/workbench";
import { summarizeModelDeltas, summarizeWorkbenchEvent } from "@/lib/agent-events";
import type { AgentRunEventResp, AgentRunResp, WorkbenchContext } from "@/types/agent";
import type { CapabilityItemResp, McpToolResp } from "@/types/capability";

const navigationItems = [
  { label: "新对话", icon: SquarePen, active: true },
  { label: "搜索", icon: Search },
  { label: "插件", icon: Blocks },
  { label: "自动化", icon: Clock3 }
];

const pinnedProjects = [{ name: "zhiman" }];

const projects = [
  { name: "ai-edu", state: "normal" },
  { name: "macos-app-console", state: "linked" },
  { name: "novex-agent", state: "current" },
  { name: "codex-sentinel", state: "normal" },
  { name: "codex-usecase", state: "normal" },
  { name: "file_agent", state: "linked" },
  { name: "pixelsquad", state: "normal" },
  { name: "aether-loom", state: "linked" },
  { name: "aimanju", state: "normal" },
  { name: "zhiman-document", state: "normal" }
];

const sessions = [
  { title: "检查 Novex 需求完成情况", age: "", active: true },
  { title: "检查 Novex main 分支", age: "19 小时" },
  { title: "检查 main 分支需求完成度", age: "20 小时" },
  { title: "检查 main 分支需求完成度", age: "1 天" },
  { title: "推进 M0-M5 需求", age: "1 天" }
];

const suggestions = [
  "Finish the one-command training-web POC launcher on main",
  "Add the missing parser queue acceptance test for today's merge",
  "Remove the last M5 operator-only gap from template apply",
  "将你常用的应用连接到 Agent"
];

const commandItems = [
  { name: "MCP", description: "显示 MCP 服务器状态", icon: Blocks },
  { name: "个性", description: "选择 Agent 的回应方式", icon: Circle },
  { name: "反馈", description: "发送有关此聊天的反馈", icon: MessageCircle },
  { name: "宠物", description: "唤醒或收起桌面宠物", icon: PanelLeft },
  { name: "推理模式", description: "超高", icon: Settings },
  { name: "模型", description: "GPT-5.5", icon: FolderGit2 },
  { name: "状态", description: "显示对话 ID、上下文使用情况及额度限制", icon: Clock3 },
  { name: "目标", description: "设置 Agent 将持续努力实现的目标", icon: ShieldAlert },
  { name: "聊天", description: "不在项目中工作", icon: MessageCircle },
  { name: "计划模式", description: "开启计划模式", icon: SquarePen },
  { name: "记忆", description: "使用开，生成开", icon: Blocks }
];

type WorkbenchUploadedFile = {
  id: string;
  name: string;
  datasetId: number;
  documentId?: number;
  fileId?: number;
  parseJobId?: number;
  status: "uploading" | "parsing" | "indexed" | "failed" | "unavailable";
  message?: string;
};

export function CodexPocApp() {
  return (
    <main className="flex min-h-screen overflow-hidden bg-[#F3ECEC] text-[#111111]">
      <Sidebar />
      <section className="relative min-w-0 flex-1 p-0">
        <TopRightControls />
        <div className="min-h-screen rounded-tl-[18px] border-l border-t border-[#E5E5E5] bg-white">
          <div className="mx-auto min-h-screen w-full max-w-[1180px] px-8 pb-12 pt-[22vh]">
            <div className="w-full">
              <h1 className="text-center text-[30px] font-medium leading-tight text-[#111111]">
                我们应该在当前项目中做些什么？
              </h1>
              <Composer />
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}

function Sidebar() {
  return (
    <aside className="flex h-screen w-[306px] shrink-0 flex-col bg-[#F3ECEC] px-[18px] pb-5 pt-[18px] text-[15px] text-[#111111]">
      <div className="mb-6 flex items-center justify-center gap-6 text-[#8A8A8A]">
        <IconButton label="切换侧栏">
          <PanelLeft aria-hidden="true" className="h-[18px] w-[18px]" strokeWidth={1.8} />
        </IconButton>
        <IconButton label="后退">
          <ArrowLeft aria-hidden="true" className="h-[18px] w-[18px]" strokeWidth={1.8} />
        </IconButton>
        <IconButton label="前进">
          <ArrowRight aria-hidden="true" className="h-[18px] w-[18px]" strokeWidth={1.8} />
        </IconButton>
      </div>

      <nav aria-label="主导航" className="space-y-2">
        {navigationItems.map((item) => (
          <button
            className={[
              "flex h-9 w-full items-center gap-3 rounded-[10px] px-1.5 text-left text-[16px] font-medium transition-colors",
              item.active ? "text-[#111111]" : "text-[#3f3b3b] hover:bg-[#F1F1F1]"
            ].join(" ")}
            key={item.label}
            type="button"
          >
            <item.icon aria-hidden="true" className="h-[18px] w-[18px] shrink-0" strokeWidth={1.9} />
            <span>{item.label}</span>
          </button>
        ))}
      </nav>

      <div className="mt-8 min-h-0 flex-1 overflow-hidden">
        <SidebarGroup title="置顶">
          {pinnedProjects.map((project) => (
            <ProjectRow key={project.name} name={project.name} />
          ))}
        </SidebarGroup>

        <SidebarGroup className="mt-7" title="项目">
          {projects.slice(0, 3).map((project) => (
            <ProjectRow active={project.state === "current"} key={project.name} linked={project.state === "linked"} name={project.name} />
          ))}
          <div className="mt-2 space-y-1 pl-[30px]">
            {sessions.map((session) => (
              <button
                className={[
                  "group flex h-9 w-full items-center justify-between gap-2 rounded-[8px] px-0 text-left text-[15px] transition-colors",
                  session.active ? "font-medium text-[#242424]" : "text-[#333333] hover:bg-[#F1F1F1]"
                ].join(" ")}
                key={`${session.title}-${session.age}`}
                type="button"
              >
                <span className="min-w-0 truncate">{session.title}</span>
                {session.active ? (
                  <span className="mr-1 h-[9px] w-[9px] shrink-0 rounded-full bg-[#0A84FF]" />
                ) : (
                  <span className="shrink-0 text-[#8A8A8A]">{session.age}</span>
                )}
              </button>
            ))}
            <button className="h-8 rounded-[8px] text-left text-[14px] font-medium text-[#8A8A8A] hover:text-[#666666]" type="button">
              展开显示
            </button>
          </div>

          {projects.slice(3).map((project) => (
            <ProjectRow key={project.name} linked={project.state === "linked"} name={project.name} />
          ))}
          <div className="pl-[30px] text-[14px] text-[#B8B8B8]">暂无对话</div>
        </SidebarGroup>

        <SidebarGroup className="mt-7" title="对话">
          <div className="h-8" />
        </SidebarGroup>
      </div>

      <button className="mt-4 flex h-10 w-full items-center gap-3 rounded-[10px] px-1.5 text-left text-[16px] font-medium hover:bg-[#F1F1F1]" type="button">
        <Settings aria-hidden="true" className="h-[19px] w-[19px]" strokeWidth={1.9} />
        设置
      </button>
    </aside>
  );
}

function SidebarGroup({
  children,
  className = "",
  title
}: {
  children: React.ReactNode;
  className?: string;
  title: string;
}) {
  return (
    <section className={className}>
      <h2 className="mb-2 px-0.5 text-[15px] font-medium text-[#9A9A9A]">{title}</h2>
      <div className="space-y-1">{children}</div>
    </section>
  );
}

function ProjectRow({ active = false, linked = false, name }: { active?: boolean; linked?: boolean; name: string }) {
  const Icon = linked ? FolderGit2 : Folder;

  return (
    <button
      className={[
        "flex h-9 w-full items-center gap-3 rounded-[8px] px-0.5 text-left text-[15px] transition-colors hover:bg-[#F1F1F1]",
        active ? "font-medium text-[#111111]" : "font-normal text-[#5E5A5A]"
      ].join(" ")}
      type="button"
    >
      <Icon aria-hidden="true" className="h-[18px] w-[18px] shrink-0 text-[#6E6969]" strokeWidth={1.8} />
      <span className="min-w-0 truncate">{name}</span>
    </button>
  );
}

function Composer() {
  const [composerValue, setComposerValue] = useState("");
  const [isCommandMenuOpen, setIsCommandMenuOpen] = useState(false);
  const [activeCommandIndex, setActiveCommandIndex] = useState(0);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [runResult, setRunResult] = useState<AgentRunResp | null>(null);
  const [runEvents, setRunEvents] = useState<AgentRunEventResp[]>([]);
  const [runError, setRunError] = useState<string | null>(null);
  const [skills, setSkills] = useState<CapabilityItemResp[]>([]);
  const [mcpTools, setMcpTools] = useState<McpToolResp[]>([]);
  const [selectedSkillCodes, setSelectedSkillCodes] = useState<string[]>([]);
  const [selectedMcpToolCodes, setSelectedMcpToolCodes] = useState<string[]>([]);
  const [webSearchEnabled, setWebSearchEnabled] = useState(false);
  const [uploadedFiles, setUploadedFiles] = useState<WorkbenchUploadedFile[]>([]);
  const [capabilityError, setCapabilityError] = useState<string | null>(null);
  const modelDeltaSummary = useMemo(() => summarizeModelDeltas(runEvents), [runEvents]);
  const eventEvidence = useMemo(() => runEvents.map(summarizeWorkbenchEvent), [runEvents]);

  useEffect(() => {
    let cancelled = false;

    async function loadCapabilities() {
      try {
        const [skillPage, serverPage] = await Promise.all([
          listSkills({ page: 1, size: 20 }),
          listMcpServers({ page: 1, size: 20 })
        ]);
        if (cancelled) {
          return;
        }
        setSkills(skillPage.list);

        const activeServers = serverPage.list.filter((server) => server.status !== 0);
        const toolGroups = await Promise.all(
          activeServers.map((server) => listMcpTools(server.id).catch(() => []))
        );
        if (!cancelled) {
          setMcpTools(toolGroups.flat().filter((tool) => tool.status !== 0));
        }
      } catch (error) {
        if (!cancelled) {
          setCapabilityError(error instanceof Error ? error.message : "Capabilities unavailable");
        }
      }
    }

    void loadCapabilities();

    return () => {
      cancelled = true;
    };
  }, []);

  function handleComposerChange(value: string) {
    setComposerValue(value);
    if (value.includes("/")) {
      setIsCommandMenuOpen(true);
      setActiveCommandIndex(0);
    }
  }

  function handleComposerKeyDown(event: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (!isCommandMenuOpen) {
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveCommandIndex((current) => (current + 1) % commandItems.length);
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveCommandIndex((current) => (current - 1 + commandItems.length) % commandItems.length);
    }

    if (event.key === "Escape") {
      event.preventDefault();
      setIsCommandMenuOpen(false);
    }

    if (event.key === "Enter") {
      event.preventDefault();
      setComposerValue(`/${commandItems[activeCommandIndex].name} `);
      setIsCommandMenuOpen(false);
    }
  }

  async function handleSubmit() {
    const input = composerValue.trim();
    if (!input || isSubmitting) {
      setRunError("请输入任务");
      return;
    }

    setIsSubmitting(true);
    setRunError(null);
    setRunResult(null);
    setRunEvents([]);
    try {
      const result = await createConfiguredModelAgentRun(input, buildWorkbenchContext());
      setRunResult(result);
      try {
        const eventPage = await listAgentRunEvents(result.runId, { page: 1, size: 100 });
        setRunEvents(eventPage.list);
      } catch {
        setRunEvents([]);
      }
    } catch (error) {
      setRunError(error instanceof Error ? error.message : "提交失败");
    } finally {
      setIsSubmitting(false);
    }
  }

  async function handleUploadFiles(fileList: FileList | File[] | null) {
    const files = Array.from(fileList ?? []);
    if (files.length === 0) {
      return;
    }

    try {
      const dataset = await ensureWorkbenchDataset();
      for (const file of files) {
        const localId = `${file.name}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
        setUploadedFiles((items) => [
          ...items,
          { id: localId, name: file.name, datasetId: dataset.id, status: "uploading" }
        ]);
        try {
          const uploaded = await uploadKnowledgeFile(dataset.id, file);
          const nextStatus = uploaded.parseJob.status >= 2 ? "indexed" : "parsing";
          setUploadedFiles((items) =>
            items.map((item) =>
              item.id === localId
                ? {
                    ...item,
                    documentId: uploaded.parseJob.documentId,
                    fileId: uploaded.file.id,
                    parseJobId: uploaded.parseJob.id,
                    status: nextStatus
                  }
                : item
            )
          );
        } catch (error) {
          setUploadedFiles((items) =>
            items.map((item) =>
              item.id === localId
                ? {
                    ...item,
                    status: "failed",
                    message: error instanceof Error ? error.message : "Upload failed"
                  }
                : item
            )
          );
        }
      }
    } catch (error) {
      setRunError(error instanceof Error ? error.message : "无法准备文件数据集");
    }
  }

  function buildWorkbenchContext(): WorkbenchContext {
    const activeFiles = uploadedFiles.filter((file) =>
      ["indexed", "parsing"].includes(file.status)
    );
    const datasetId = activeFiles.find((file) => file.datasetId)?.datasetId;

    return {
      mode: "agent",
      ...(datasetId ? { datasetId } : {}),
      documentIds: activeFiles.flatMap((file) => (file.documentId ? [file.documentId] : [])),
      fileIds: activeFiles.flatMap((file) => (file.fileId ? [file.fileId] : [])),
      skillCodes: selectedSkillCodes,
      mcpToolCodes: selectedMcpToolCodes,
      webSearchEnabled
    };
  }

  function toggleSkill(code: string) {
    setSelectedSkillCodes((codes) => toggleString(codes, code));
  }

  function toggleMcpTool(code: string) {
    setSelectedMcpToolCodes((codes) => toggleString(codes, code));
  }

  return (
    <div className="relative mt-12 w-full">
      {isCommandMenuOpen ? <CommandMenu activeIndex={activeCommandIndex} /> : null}
      <div className="rounded-[27px] bg-[#F6F6F6] pb-0 shadow-[0_14px_34px_rgba(17,17,17,0.06)]">
        <div className="rounded-[27px] border border-[#DCDCDC] bg-white p-4 shadow-[0_2px_9px_rgba(17,17,17,0.03)]">
          <label className="sr-only" htmlFor="task-input">
            任务输入
          </label>
          <textarea
            className="min-h-[182px] w-full resize-none bg-transparent px-0.5 py-0 text-[16px] leading-7 text-[#111111] outline-none placeholder:text-[#9A9A9A]"
            id="task-input"
            onChange={(event) => handleComposerChange(event.target.value)}
            onKeyDown={handleComposerKeyDown}
            placeholder="描述你希望 Agent 在当前仓库中完成的任务，或输入 / 打开命令菜单"
            value={composerValue}
          />
          <div className="mt-3 flex items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <button
                aria-label="添加附件"
                className="flex h-8 w-8 items-center justify-center rounded-full text-[#8A8A8A] hover:bg-[#F1F1F1] hover:text-[#111111]"
                type="button"
              >
                <Plus aria-hidden="true" className="h-[21px] w-[21px]" strokeWidth={1.9} />
              </button>
              <button
                className="inline-flex h-8 items-center gap-1.5 rounded-[9px] px-2 text-[15px] font-medium text-[#F97316] hover:bg-[#FFF3EA]"
                type="button"
              >
                <ShieldAlert aria-hidden="true" className="h-[17px] w-[17px]" strokeWidth={2} />
                完全访问
                <ChevronDown aria-hidden="true" className="h-[15px] w-[15px]" strokeWidth={2} />
              </button>
            </div>
            <div className="flex shrink-0 items-center gap-3">
              <button className="inline-flex h-8 items-center gap-1 rounded-[9px] px-2 text-[15px] text-[#111111] hover:bg-[#F1F1F1]" type="button">
                <span>5.5</span>
                <span className="text-[#8A8A8A]">超高</span>
                <ChevronDown aria-hidden="true" className="h-[15px] w-[15px]" strokeWidth={2} />
              </button>
              <button
                aria-label="发送"
                className="flex h-[42px] w-[42px] items-center justify-center rounded-full bg-[#050505] text-white shadow-sm hover:bg-[#222222] disabled:cursor-not-allowed disabled:bg-[#B8B8B8]"
                disabled={isSubmitting}
                onClick={handleSubmit}
                type="button"
              >
                <ArrowUp aria-hidden="true" className="h-[22px] w-[22px]" strokeWidth={2.2} />
              </button>
            </div>
          </div>
        </div>
        <button
          className="flex h-12 w-full items-center gap-2 rounded-b-[27px] px-5 text-left text-[15px] text-[#8A8A8A] hover:text-[#666666]"
          type="button"
        >
          <Folder aria-hidden="true" className="h-[18px] w-[18px]" strokeWidth={1.8} />
          <span>novex-agent</span>
          <ChevronDown aria-hidden="true" className="h-[15px] w-[15px]" strokeWidth={1.9} />
        </button>
      </div>

      <ContextPanel
        capabilityError={capabilityError}
        mcpTools={mcpTools}
        onToggleMcpTool={toggleMcpTool}
        onToggleSkill={toggleSkill}
        onToggleWebSearch={() => setWebSearchEnabled((enabled) => !enabled)}
        onUploadFiles={handleUploadFiles}
        selectedMcpToolCodes={selectedMcpToolCodes}
        selectedSkillCodes={selectedSkillCodes}
        skills={skills}
        uploadedFiles={uploadedFiles}
        webSearchEnabled={webSearchEnabled}
      />

      {runError ? (
        <p className="mt-4 rounded-[10px] border border-[#F3B5B5] bg-[#FFF5F5] px-4 py-3 text-[14px] text-[#A12626]" role="alert">
          {runError}
        </p>
      ) : null}

      {runResult ? (
        <section aria-live="polite" className="mt-4 rounded-[10px] border border-[#E5E5E5] bg-[#FAFAFA] px-4 py-3 text-[14px] text-[#333333]">
          <div className="flex flex-wrap items-center gap-2 text-[#6F6F6F]">
            <span>Run #{runResult.runId}</span>
            <span>{runResult.status}</span>
            <span>{runResult.traceId}</span>
          </div>
          <p className="mt-2 whitespace-pre-wrap text-[15px] leading-6 text-[#111111]">
            {runResult.finalOutput || `Agent run ${runResult.status}`}
          </p>
          {modelDeltaSummary ? (
            <section className="mt-3 rounded-[9px] border border-[#D7E7FF] bg-white px-3 py-3">
              <div className="flex flex-wrap items-center justify-between gap-2 text-[13px] text-[#596A7E]">
                <span className="font-medium text-[#111111]">Live model output</span>
                <span>{modelDeltaSummary.chunkCount} chunks</span>
              </div>
              <p className="mt-2 whitespace-pre-wrap text-[15px] leading-6 text-[#111111]">
                {modelDeltaSummary.text}
              </p>
              <div className="mt-2 flex flex-wrap gap-2 text-[12px] text-[#6F6F6F]">
                {modelDeltaSummary.routeId ? <span>{modelDeltaSummary.routeId}</span> : null}
                {modelDeltaSummary.model ? <span>{modelDeltaSummary.model}</span> : null}
              </div>
            </section>
          ) : null}
          {eventEvidence.length > 0 ? (
            <section className="mt-3 space-y-2 rounded-[9px] border border-[#E8E8E8] bg-white px-3 py-3">
              <h3 className="text-[13px] font-medium text-[#111111]">Run evidence</h3>
              {eventEvidence.map((evidence) => (
                <article
                  className="rounded-[8px] border border-[#EFEFEF] px-3 py-2"
                  key={`${evidence.sequenceNo}-${evidence.title}`}
                >
                  <div className="flex items-center justify-between gap-2 text-[13px]">
                    <strong className="font-medium text-[#111111]">{evidence.title}</strong>
                    <span className="text-[#8A8A8A]">{evidence.kind}</span>
                  </div>
                  <p className="mt-1 whitespace-pre-wrap text-[13px] leading-5 text-[#555555]">
                    {evidence.text}
                  </p>
                </article>
              ))}
            </section>
          ) : null}
        </section>
      ) : null}

      <div className="mt-7 divide-y divide-[#EEEEEE]">
        {suggestions.map((suggestion, index) => (
          <button
            className="flex h-[47px] w-full items-center gap-3 px-3 text-left text-[15px] text-[#8A8A8A] hover:bg-[#FAFAFA] hover:text-[#666666]"
            key={suggestion}
            type="button"
          >
            {index === suggestions.length - 1 ? (
              <Blocks aria-hidden="true" className="h-[17px] w-[17px] shrink-0" strokeWidth={1.8} />
            ) : (
              <MessageCircle aria-hidden="true" className="h-[18px] w-[18px] shrink-0" strokeWidth={1.8} />
            )}
            <span className="min-w-0 truncate">{suggestion}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

function ContextPanel({
  capabilityError,
  mcpTools,
  onToggleMcpTool,
  onToggleSkill,
  onToggleWebSearch,
  onUploadFiles,
  selectedMcpToolCodes,
  selectedSkillCodes,
  skills,
  uploadedFiles,
  webSearchEnabled
}: {
  capabilityError: string | null;
  mcpTools: McpToolResp[];
  onToggleMcpTool: (code: string) => void;
  onToggleSkill: (code: string) => void;
  onToggleWebSearch: () => void;
  onUploadFiles: (files: FileList | null) => void;
  selectedMcpToolCodes: string[];
  selectedSkillCodes: string[];
  skills: CapabilityItemResp[];
  uploadedFiles: WorkbenchUploadedFile[];
  webSearchEnabled: boolean;
}) {
  return (
    <aside className="mt-4 rounded-[16px] border border-[#E5E5E5] bg-[#FBFBFB] px-4 py-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-[16px] font-medium text-[#111111]">Context</h2>
        <div className="flex items-center gap-2">
          <button
            className="inline-flex h-8 items-center gap-1.5 rounded-[9px] border border-[#E5E5E5] bg-white px-2 text-[13px] text-[#333333]"
            type="button"
          >
            <FileText aria-hidden="true" className="h-4 w-4" strokeWidth={1.8} />
            Files
          </button>
          <button
            className="inline-flex h-8 items-center gap-1.5 rounded-[9px] border border-[#E5E5E5] bg-white px-2 text-[13px] text-[#333333]"
            type="button"
          >
            <Blocks aria-hidden="true" className="h-4 w-4" strokeWidth={1.8} />
            Skills
          </button>
          <button
            className="inline-flex h-8 items-center gap-1.5 rounded-[9px] border border-[#E5E5E5] bg-white px-2 text-[13px] text-[#333333]"
            type="button"
          >
            <Wrench aria-hidden="true" className="h-4 w-4" strokeWidth={1.8} />
            MCP
          </button>
        </div>
      </div>

      <div className="mt-4 grid gap-4 md:grid-cols-2">
        <section>
          <div className="mb-2 flex items-center justify-between gap-2">
            <h3 className="text-[13px] font-medium text-[#6F6F6F]">Files</h3>
            <button
              aria-label="选择文件"
              className="inline-flex h-7 w-7 items-center justify-center rounded-full border border-[#E5E5E5] bg-white text-[#555555] hover:bg-[#F1F1F1]"
              onClick={() => document.getElementById("workbench-file-input")?.click()}
              type="button"
            >
              <Plus aria-hidden="true" className="h-4 w-4" strokeWidth={1.9} />
            </button>
            <input
              aria-label="Upload files"
              className="sr-only"
              id="workbench-file-input"
              multiple
              onChange={(event) => onUploadFiles(event.currentTarget.files)}
              type="file"
            />
          </div>
          <div className="flex min-h-8 flex-wrap gap-2">
            {uploadedFiles.length > 0 ? (
              uploadedFiles.map((file) => (
                <span
                  className={[
                    "inline-flex max-w-full items-center gap-1 rounded-[8px] border px-2 py-1 text-[13px]",
                    file.status === "failed"
                      ? "border-[#F3B5B5] bg-[#FFF5F5] text-[#A12626]"
                      : "border-[#D7E7FF] bg-white text-[#333333]"
                  ].join(" ")}
                  key={file.id}
                >
                  <FileText aria-hidden="true" className="h-3.5 w-3.5 shrink-0" />
                  <span className="min-w-0 truncate">{file.name}</span>
                  <span className="text-[#8A8A8A]">{file.status}</span>
                </span>
              ))
            ) : (
              <span className="text-[13px] text-[#9A9A9A]">No files</span>
            )}
          </div>
        </section>

        <section>
          <h3 className="mb-2 text-[13px] font-medium text-[#6F6F6F]">Skills</h3>
          <div className="flex min-h-8 flex-wrap gap-2">
            {skills.length > 0 ? (
              skills.map((skill) => {
                const selected = selectedSkillCodes.includes(skill.code);
                return (
                  <button
                    className={[
                      "rounded-[8px] border px-2 py-1 text-[13px]",
                      selected
                        ? "border-[#0A84FF] bg-[#F3F8FF] text-[#0A54A8]"
                        : "border-[#E5E5E5] bg-white text-[#333333]"
                    ].join(" ")}
                    key={skill.code}
                    onClick={() => onToggleSkill(skill.code)}
                    type="button"
                  >
                    {skill.name || skill.code}
                  </button>
                );
              })
            ) : (
              <span className="text-[13px] text-[#9A9A9A]">No skills</span>
            )}
          </div>
        </section>

        <section>
          <h3 className="mb-2 text-[13px] font-medium text-[#6F6F6F]">MCP</h3>
          <div className="flex min-h-8 flex-wrap gap-2">
            {mcpTools.length > 0 ? (
              mcpTools.map((tool) => {
                const selected = selectedMcpToolCodes.includes(tool.toolCode);
                return (
                  <button
                    className={[
                      "rounded-[8px] border px-2 py-1 text-[13px]",
                      selected
                        ? "border-[#0A84FF] bg-[#F3F8FF] text-[#0A54A8]"
                        : "border-[#E5E5E5] bg-white text-[#333333]"
                    ].join(" ")}
                    key={tool.toolCode}
                    onClick={() => onToggleMcpTool(tool.toolCode)}
                    type="button"
                  >
                    {tool.toolName || tool.toolCode}
                  </button>
                );
              })
            ) : (
              <span className="text-[13px] text-[#9A9A9A]">No MCP tools</span>
            )}
          </div>
        </section>

        <section>
          <h3 className="mb-2 text-[13px] font-medium text-[#6F6F6F]">Search</h3>
          <button
            aria-checked={webSearchEnabled}
            aria-label="Web search"
            className={[
              "inline-flex h-8 items-center gap-2 rounded-[9px] border px-2 text-[13px]",
              webSearchEnabled
                ? "border-[#0A84FF] bg-[#F3F8FF] text-[#0A54A8]"
                : "border-[#E5E5E5] bg-white text-[#333333]"
            ].join(" ")}
            onClick={onToggleWebSearch}
            role="switch"
            type="button"
          >
            <Globe2 aria-hidden="true" className="h-4 w-4" strokeWidth={1.8} />
            Search web
          </button>
          <p className="mt-2 text-[12px] text-[#8A8A8A]">
            {webSearchEnabled ? "enabled / dry-run capable" : "disabled"}
          </p>
        </section>
      </div>

      {capabilityError ? (
        <p className="mt-3 rounded-[8px] border border-[#F3B5B5] bg-white px-3 py-2 text-[12px] text-[#A12626]">
          {capabilityError}
        </p>
      ) : null}
    </aside>
  );
}

function toggleString(values: string[], value: string) {
  return values.includes(value)
    ? values.filter((current) => current !== value)
    : [...values, value];
}

function CommandMenu({ activeIndex }: { activeIndex: number }) {
  return (
    <div
      aria-label="命令菜单"
      className="absolute bottom-[calc(100%+14px)] left-0 right-0 z-20 overflow-y-auto rounded-[26px] border border-[#E3E3E3] bg-white p-2 shadow-[0_18px_48px_rgba(17,17,17,0.12)]"
      role="listbox"
      style={{ maxHeight: "min(390px, calc(100vh - 580px))" }}
    >
      {commandItems.map((item, index) => (
        <div
          aria-label={item.name}
          aria-selected={index === activeIndex}
          className={[
            "flex h-[54px] items-center gap-4 rounded-[18px] px-4 text-left transition-colors",
            index === activeIndex ? "bg-[#F1F1F1]" : "hover:bg-[#F7F7F7]"
          ].join(" ")}
          key={item.name}
          role="option"
        >
          <item.icon aria-hidden="true" className="h-[22px] w-[22px] shrink-0 text-[#6F6F6F]" strokeWidth={1.85} />
          <span className="min-w-0 shrink-0 text-[20px] font-semibold text-[#111111]">{item.name}</span>
          <span className="min-w-0 truncate text-[18px] text-[#8A8A8A]">{item.description}</span>
        </div>
      ))}
    </div>
  );
}

function IconButton({ children, label }: { children: React.ReactNode; label: string }) {
  return (
    <button aria-label={label} className="flex h-7 w-7 items-center justify-center rounded-[8px] hover:bg-[#EDE7E7]" type="button">
      {children}
    </button>
  );
}

function TopRightControls() {
  return (
    <div className="pointer-events-none absolute right-7 top-5 z-10 flex items-center gap-4 text-[#8A8A8A]">
      <div className="pointer-events-auto flex h-8 items-center gap-2 rounded-[12px] border border-[#E5E5E5] bg-white px-2 shadow-sm">
        <span className="flex h-5 w-5 items-center justify-center rounded-[6px] bg-[#F7FAFF] text-[11px] font-semibold text-[#0A84FF]">V</span>
        <ChevronDown aria-hidden="true" className="h-[14px] w-[14px]" strokeWidth={1.8} />
      </div>
      <Circle aria-hidden="true" className="h-[17px] w-[17px]" strokeWidth={1.8} />
      <PanelLeft aria-hidden="true" className="h-[17px] w-[17px]" strokeWidth={1.8} />
    </div>
  );
}
