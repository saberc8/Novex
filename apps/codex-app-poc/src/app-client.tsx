"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowUp,
  Blocks,
  Check,
  ChevronDown,
  FileText,
  Folder,
  FolderGit2,
  Globe2,
  Plus,
  Wrench
} from "lucide-react";
import { createConfiguredModelAgentRun, listAgentRunEvents } from "@/api/agent";
import { listMcpServers, listMcpTools, listSkills } from "@/api/capability";
import { uploadKnowledgeFile } from "@/api/knowledge";
import { ensureWorkbenchDataset } from "@/api/workbench";
import { summarizeModelDeltas, summarizeWorkbenchEvent } from "@/lib/agent-events";
import type { ModelDeltaSummary, WorkbenchEventEvidence } from "@/lib/agent-events";
import type { AgentRunEventResp, AgentRunResp, WorkbenchContext } from "@/types/agent";
import type { CapabilityItemResp, McpToolResp } from "@/types/capability";

const CURRENT_PROJECT_NAME = "novex-agent";
const DEFAULT_MODEL_ROUTE_ID = "runtime.llm";
const PROJECT_STORAGE_KEY = "novex_codex_poc_projects_v1";
const ACTIVE_PROJECT_STORAGE_KEY = "novex_codex_poc_active_project_v1";

type ConversationSummary = {
  id: string;
  title: string;
  age: string;
  status: string;
};

type ConversationTurn = {
  id: string;
  prompt: string;
  modelRouteId: string;
  runResult: AgentRunResp | null;
  runEvents: AgentRunEventResp[];
  runError: string | null;
};

type ProjectRecord = {
  id: string;
  name: string;
  turns: ConversationTurn[];
};

type ModelRouteOption = {
  routeId: string;
  label: string;
};

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
  const [projects, setProjects] = useState<ProjectRecord[]>(() => readStoredProjects());
  const [activeProjectId, setActiveProjectId] = useState(() => readStoredActiveProjectId());
  const activeProject = useMemo(
    () => projects.find((project) => project.id === activeProjectId) ?? projects[0],
    [activeProjectId, projects]
  );
  const activeConversationId = activeProject?.turns.at(-1)?.id ?? null;
  const conversations = useMemo(
    () => conversationSummariesForProject(activeProject),
    [activeProject]
  );

  useEffect(() => {
    writeStoredProjects(projects);
  }, [projects]);

  useEffect(() => {
    writeStoredActiveProjectId(activeProject?.id ?? null);
  }, [activeProject?.id]);

  function updateProject(projectId: string, updater: (project: ProjectRecord) => ProjectRecord) {
    setProjects((items) =>
      items.map((project) => (project.id === projectId ? updater(project) : project))
    );
  }

  function handleNewProject() {
    const nextProject: ProjectRecord = {
      id: `project-${Date.now()}`,
      name: `新项目 ${nextProjectNumber(projects)}`,
      turns: []
    };
    setProjects((items) => [nextProject, ...items]);
    setActiveProjectId(nextProject.id);
  }

  return (
    <main className="flex min-h-screen overflow-hidden bg-[#F4F2F1] text-[#111111]">
      <Sidebar
        activeConversationId={activeConversationId}
        activeProjectId={activeProject?.id ?? null}
        conversations={conversations}
        onNewProject={handleNewProject}
        onProjectSelect={setActiveProjectId}
        projects={projects}
      />
      <section className="relative min-w-0 flex-1 p-0">
        <div className="min-h-screen rounded-tl-[18px] border-l border-t border-[#E5E5E5] bg-white">
          <Workbench
            activeConversationId={activeConversationId}
            activeProject={activeProject}
            onProjectUpdate={updateProject}
          />
        </div>
      </section>
    </main>
  );
}

function Sidebar({
  activeConversationId,
  activeProjectId,
  conversations,
  onNewProject,
  onProjectSelect,
  projects
}: {
  activeConversationId: string | null;
  activeProjectId: string | null;
  conversations: ConversationSummary[];
  onNewProject: () => void;
  onProjectSelect: (id: string) => void;
  projects: ProjectRecord[];
}) {
  return (
    <aside className="flex h-screen w-[306px] shrink-0 flex-col bg-[#F4F2F1] px-[18px] pb-5 pt-[18px] text-[15px] text-[#111111]">
      <div className="min-h-0 flex-1 overflow-hidden">
        <SidebarGroup
          action={
            <IconButton label="新建项目" onClick={onNewProject}>
              <Plus aria-hidden="true" className="h-[17px] w-[17px]" strokeWidth={1.9} />
            </IconButton>
          }
          title="项目"
        >
          {projects.map((project) => (
            <ProjectRow
              active={project.id === activeProjectId}
              key={project.id}
              name={project.name}
              onClick={() => onProjectSelect(project.id)}
            />
          ))}
        </SidebarGroup>

        <SidebarGroup className="mt-7" title="对话">
          {conversations.length > 0 ? (
            conversations.map((conversation) => (
              <button
                className={[
                  "group flex h-9 w-full items-center justify-between gap-2 rounded-[8px] px-1 text-left text-[15px] transition-colors",
                  conversation.id === activeConversationId
                    ? "bg-[#EAEAEA] font-medium text-[#242424]"
                    : "text-[#333333] hover:bg-[#EBE8E6]"
                ].join(" ")}
                key={conversation.id}
                type="button"
              >
                <span className="min-w-0 truncate">{conversation.title}</span>
                {conversation.id === activeConversationId ? (
                  <span className="mr-1 h-[9px] w-[9px] shrink-0 rounded-full bg-[#0A84FF]" />
                ) : (
                  <span className="shrink-0 text-[#8A8A8A]">{conversation.age}</span>
                )}
              </button>
            ))
          ) : (
            <div className="px-1 text-[14px] text-[#A0A0A0]">暂无对话</div>
          )}
        </SidebarGroup>
      </div>
    </aside>
  );
}

function SidebarGroup({
  action,
  children,
  className = "",
  title
}: {
  action?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  title: string;
}) {
  return (
    <section className={className}>
      <div className="mb-2 flex items-center justify-between gap-2">
        <h2 className="px-0.5 text-[15px] font-medium text-[#9A9A9A]">{title}</h2>
        {action}
      </div>
      <div className="space-y-1">{children}</div>
    </section>
  );
}

function ProjectRow({
  active = false,
  linked = false,
  name,
  onClick
}: {
  active?: boolean;
  linked?: boolean;
  name: string;
  onClick?: () => void;
}) {
  const Icon = linked ? FolderGit2 : Folder;

  return (
    <button
      className={[
        "flex h-9 w-full items-center gap-3 rounded-[8px] px-0.5 text-left text-[15px] transition-colors hover:bg-[#EBE8E6]",
        active ? "font-medium text-[#111111]" : "font-normal text-[#5E5A5A]"
      ].join(" ")}
      onClick={onClick}
      type="button"
    >
      <Icon aria-hidden="true" className="h-[18px] w-[18px] shrink-0 text-[#6E6969]" strokeWidth={1.8} />
      <span className="min-w-0 truncate">{name}</span>
    </button>
  );
}

function Workbench({
  activeConversationId,
  activeProject,
  onProjectUpdate
}: {
  activeConversationId: string | null;
  activeProject: ProjectRecord | undefined;
  onProjectUpdate: (projectId: string, updater: (project: ProjectRecord) => ProjectRecord) => void;
}) {
  const [composerValue, setComposerValue] = useState("");
  const [composerError, setComposerError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [skills, setSkills] = useState<CapabilityItemResp[]>([]);
  const [mcpTools, setMcpTools] = useState<McpToolResp[]>([]);
  const [selectedSkillCodes, setSelectedSkillCodes] = useState<string[]>([]);
  const [selectedMcpToolCodes, setSelectedMcpToolCodes] = useState<string[]>([]);
  const [webSearchEnabled, setWebSearchEnabled] = useState(false);
  const [uploadedFiles, setUploadedFiles] = useState<WorkbenchUploadedFile[]>([]);
  const [capabilityError, setCapabilityError] = useState<string | null>(null);
  const modelOptions = useMemo(() => configuredModelRouteOptions(), []);
  const [selectedModelRouteId, setSelectedModelRouteId] = useState(
    modelOptions[0]?.routeId ?? DEFAULT_MODEL_ROUTE_ID
  );
  const selectedModel =
    modelOptions.find((option) => option.routeId === selectedModelRouteId) ?? modelOptions[0];
  const turns = activeProject?.turns ?? [];
  const latestTurn = turns.at(-1) ?? null;
  const runResult = latestTurn?.runResult ?? null;
  const runEvents = latestTurn?.runEvents ?? [];
  const runError = latestTurn?.runError ?? composerError;
  const modelDeltaSummary = useMemo(() => summarizeModelDeltas(runEvents), [runEvents]);
  const eventEvidence = useMemo(() => runEvents.map(summarizeWorkbenchEvent), [runEvents]);
  const hasConversation = turns.length > 0 || Boolean(runError) || isSubmitting;
  const conversationTitle = latestTurn
    ? conversationTitleFromPrompt(latestTurn.prompt)
    : activeProject?.name ?? "新对话";

  useEffect(() => {
    setComposerValue("");
    setComposerError(null);
  }, [activeProject?.id]);

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
  }

  async function handleSubmit() {
    const input = composerValue.trim();
    if (!input || isSubmitting) {
      setComposerError("请输入任务");
      return;
    }
    if (!activeProject) {
      setComposerError("请先创建项目");
      return;
    }

    const projectId = activeProject.id;
    const turnId = `conversation-${Date.now()}`;
    const modelRouteId = selectedModelRouteId;
    const nextTurn: ConversationTurn = {
      id: turnId,
      prompt: input,
      modelRouteId,
      runResult: null,
      runEvents: [],
      runError: null
    };
    setComposerValue("");
    setIsSubmitting(true);
    setComposerError(null);
    onProjectUpdate(projectId, (project) => ({
      ...project,
      turns: [...project.turns, nextTurn]
    }));

    try {
      const result = await createConfiguredModelAgentRun(input, buildWorkbenchContext());
      let nextRunEvents: AgentRunEventResp[] = [];
      try {
        const eventPage = await listAgentRunEvents(result.runId, { page: 1, size: 100 });
        nextRunEvents = eventPage.list;
      } catch {
        nextRunEvents = [];
      }
      onProjectUpdate(projectId, (project) => updateProjectTurn(project, turnId, {
        runResult: result,
        runEvents: nextRunEvents
      }));
    } catch (error) {
      const message = error instanceof Error ? error.message : "提交失败";
      onProjectUpdate(projectId, (project) => updateProjectTurn(project, turnId, {
        runError: message
      }));
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
      setComposerError(error instanceof Error ? error.message : "无法准备文件数据集");
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
      webSearchEnabled,
      routeId: selectedModelRouteId
    };
  }

  function toggleSkill(code: string) {
    setSelectedSkillCodes((codes) => toggleString(codes, code));
  }

  function toggleMcpTool(code: string) {
    setSelectedMcpToolCodes((codes) => toggleString(codes, code));
  }

  const composer = (
    <ComposerSurface
      capabilityError={capabilityError}
      compact={hasConversation}
      composerValue={composerValue}
      isSubmitting={isSubmitting}
      mcpTools={mcpTools}
      modelOptions={modelOptions}
      onComposerChange={handleComposerChange}
      onModelSelect={setSelectedModelRouteId}
      onSubmit={handleSubmit}
      onToggleMcpTool={toggleMcpTool}
      onToggleSkill={toggleSkill}
      onToggleWebSearch={() => setWebSearchEnabled((enabled) => !enabled)}
      onUploadFiles={handleUploadFiles}
      selectedMcpToolCodes={selectedMcpToolCodes}
      selectedModelRouteId={selectedModelRouteId}
      selectedSkillCodes={selectedSkillCodes}
      skills={skills}
      uploadedFiles={uploadedFiles}
      webSearchEnabled={webSearchEnabled}
    />
  );

  if (!hasConversation) {
    return (
      <div className="mx-auto min-h-screen w-full max-w-[1180px] px-8 pb-12 pt-[22vh]">
        <h1 className="text-center text-[30px] font-medium leading-tight text-[#111111]">
          我们应该在当前项目中做些什么？
        </h1>
        <div className="mt-12">{composer}</div>
      </div>
    );
  }

  return (
    <div className="flex h-screen bg-white">
      <section className="relative flex min-w-0 flex-1 flex-col overflow-hidden">
        <ConversationHeader title={conversationTitle} />
        <div className="min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-[1020px] px-8 pb-10 pt-8">
            <ConversationTranscript
              isSubmitting={isSubmitting}
              modelOptions={modelOptions}
              pendingError={turns.length === 0 ? runError : null}
              turns={turns}
            />
          </div>
        </div>
        <div className="shrink-0 border-t border-[#EFEFEF] bg-white/96 px-8 py-5 backdrop-blur">
          <div className="mx-auto w-full max-w-[1020px]">{composer}</div>
        </div>
      </section>
      <OutputRail
        activeConversationId={activeConversationId}
        eventEvidence={eventEvidence}
        mcpTools={mcpTools}
        modelDeltaSummary={modelDeltaSummary}
        runEvents={runEvents}
        runResult={runResult}
        selectedMcpToolCodes={selectedMcpToolCodes}
        selectedModel={selectedModel}
        selectedSkillCodes={selectedSkillCodes}
        skills={skills}
        uploadedFiles={uploadedFiles}
        webSearchEnabled={webSearchEnabled}
      />
    </div>
  );
}

function ComposerSurface({
  capabilityError,
  compact,
  composerValue,
  isSubmitting,
  mcpTools,
  modelOptions,
  onComposerChange,
  onModelSelect,
  onSubmit,
  onToggleMcpTool,
  onToggleSkill,
  onToggleWebSearch: handleToggleWebSearch,
  onUploadFiles,
  selectedMcpToolCodes,
  selectedModelRouteId,
  selectedSkillCodes,
  skills,
  uploadedFiles,
  webSearchEnabled
}: {
  capabilityError: string | null;
  compact: boolean;
  composerValue: string;
  isSubmitting: boolean;
  mcpTools: McpToolResp[];
  modelOptions: ModelRouteOption[];
  onComposerChange: (value: string) => void;
  onModelSelect: (routeId: string) => void;
  onSubmit: () => void;
  onToggleMcpTool: (code: string) => void;
  onToggleSkill: (code: string) => void;
  onToggleWebSearch: () => void;
  onUploadFiles: (files: FileList | null) => void;
  selectedMcpToolCodes: string[];
  selectedModelRouteId: string;
  selectedSkillCodes: string[];
  skills: CapabilityItemResp[];
  uploadedFiles: WorkbenchUploadedFile[];
  webSearchEnabled: boolean;
}) {
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  return (
    <div className="relative w-full">
      <div
        className={[
          "border border-[#DCDCDC] bg-white shadow-[0_14px_34px_rgba(17,17,17,0.06)]",
          compact ? "rounded-[22px]" : "rounded-[27px]"
        ].join(" ")}
      >
        <div className={compact ? "p-4" : "p-4"}>
          <label className="sr-only" htmlFor="task-input">
            任务输入
          </label>
          <textarea
            className={[
              "w-full resize-none bg-transparent px-0.5 py-0 text-[16px] leading-7 text-[#111111] outline-none placeholder:text-[#9A9A9A]",
              compact ? "min-h-[62px]" : "min-h-[182px]"
            ].join(" ")}
            id="task-input"
            onChange={(event) => onComposerChange(event.target.value)}
            placeholder="描述你希望 Agent 在当前仓库中完成的任务"
            value={composerValue}
          />
          <div className="mt-3 flex items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-2">
              <button
                aria-label="添加附件"
                className="flex h-8 w-8 items-center justify-center rounded-full text-[#8A8A8A] hover:bg-[#F1F1F1] hover:text-[#111111]"
                onClick={() => fileInputRef.current?.click()}
                type="button"
              >
                <Plus aria-hidden="true" className="h-[21px] w-[21px]" strokeWidth={1.9} />
              </button>
              <input
                aria-label="Upload files"
                className="sr-only"
                multiple
                onChange={(event) => onUploadFiles(event.currentTarget.files)}
                ref={fileInputRef}
                type="file"
              />
              <button
                aria-checked={webSearchEnabled}
                aria-label="Web search"
                className={[
                  "inline-flex h-8 items-center gap-1.5 rounded-[9px] px-2 text-[14px] font-medium transition-colors",
                  webSearchEnabled
                    ? "bg-[#EFF6FF] text-[#0A54A8]"
                    : "text-[#6F6F6F] hover:bg-[#F1F1F1] hover:text-[#111111]"
                ].join(" ")}
                onClick={handleToggleWebSearch}
                role="switch"
                type="button"
              >
                <Globe2 aria-hidden="true" className="h-[16px] w-[16px]" strokeWidth={1.9} />
                联网
              </button>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <ModelSelector
                onSelect={onModelSelect}
                options={modelOptions}
                selectedRouteId={selectedModelRouteId}
              />
              <button
                aria-label="发送"
                className="flex h-[42px] w-[42px] items-center justify-center rounded-full bg-[#050505] text-white shadow-sm hover:bg-[#222222] disabled:cursor-not-allowed disabled:bg-[#B8B8B8]"
                disabled={isSubmitting}
                onClick={onSubmit}
                type="button"
              >
                <ArrowUp aria-hidden="true" className="h-[22px] w-[22px]" strokeWidth={2.2} />
              </button>
            </div>
          </div>
        </div>
      </div>

      <ContextChipDock
        capabilityError={capabilityError}
        mcpTools={mcpTools}
        onToggleMcpTool={onToggleMcpTool}
        onToggleSkill={onToggleSkill}
        selectedMcpToolCodes={selectedMcpToolCodes}
        selectedSkillCodes={selectedSkillCodes}
        skills={skills}
        uploadedFiles={uploadedFiles}
      />
    </div>
  );
}

function ContextChipDock({
  capabilityError,
  mcpTools,
  onToggleMcpTool,
  onToggleSkill,
  selectedMcpToolCodes,
  selectedSkillCodes,
  skills,
  uploadedFiles
}: {
  capabilityError: string | null;
  mcpTools: McpToolResp[];
  onToggleMcpTool: (code: string) => void;
  onToggleSkill: (code: string) => void;
  selectedMcpToolCodes: string[];
  selectedSkillCodes: string[];
  skills: CapabilityItemResp[];
  uploadedFiles: WorkbenchUploadedFile[];
}) {
  const hasDockContent =
    uploadedFiles.length > 0 || skills.length > 0 || mcpTools.length > 0 || capabilityError !== null;

  if (!hasDockContent) {
    return null;
  }

  return (
    <div className="mt-3 flex flex-wrap items-center gap-2">
      {uploadedFiles.map((file) => (
        <span
          className={[
            "inline-flex max-w-full items-center gap-1 rounded-[8px] border px-2 py-1 text-[13px]",
            file.status === "failed"
              ? "border-[#F3B5B5] bg-[#FFF5F5] text-[#A12626]"
              : "border-[#D7E7FF] bg-[#F8FBFF] text-[#333333]"
          ].join(" ")}
          key={file.id}
        >
          <FileText aria-hidden="true" className="h-3.5 w-3.5 shrink-0" />
          <span className="min-w-0 truncate">{file.name}</span>
          <span className="text-[#8A8A8A]">{file.status}</span>
        </span>
      ))}

      {skills.map((skill) => {
        const selected = selectedSkillCodes.includes(skill.code);
        return (
          <button
            className={[
              "inline-flex h-8 items-center gap-1.5 rounded-[8px] border px-2 text-[13px]",
              selected
                ? "border-[#0A84FF] bg-[#F3F8FF] text-[#0A54A8]"
                : "border-[#E5E5E5] bg-white text-[#333333] hover:bg-[#F6F6F6]"
            ].join(" ")}
            key={skill.code}
            onClick={() => onToggleSkill(skill.code)}
            type="button"
          >
            <Blocks aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={1.8} />
            {skill.name || skill.code}
          </button>
        );
      })}

      {mcpTools.map((tool) => {
        const selected = selectedMcpToolCodes.includes(tool.toolCode);
        return (
          <button
            className={[
              "inline-flex h-8 items-center gap-1.5 rounded-[8px] border px-2 text-[13px]",
              selected
                ? "border-[#0A84FF] bg-[#F3F8FF] text-[#0A54A8]"
                : "border-[#E5E5E5] bg-white text-[#333333] hover:bg-[#F6F6F6]"
            ].join(" ")}
            key={tool.toolCode}
            onClick={() => onToggleMcpTool(tool.toolCode)}
            type="button"
          >
            <Wrench aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={1.8} />
            {tool.toolName || tool.toolCode}
          </button>
        );
      })}

      {capabilityError ? (
        <span className="rounded-[8px] border border-[#F3B5B5] bg-white px-2 py-1 text-[12px] text-[#A12626]">
          {capabilityError}
        </span>
      ) : null}
    </div>
  );
}

function ConversationHeader({ title }: { title: string }) {
  return (
    <header className="flex h-[62px] items-center border-b border-[#EFEFEF] px-6">
      <div className="flex min-w-0 items-center gap-3">
        <h1 className="min-w-0 truncate text-[17px] font-semibold text-[#111111]">{title}</h1>
      </div>
    </header>
  );
}

function ConversationTranscript({
  isSubmitting,
  modelOptions,
  pendingError,
  turns
}: {
  isSubmitting: boolean;
  modelOptions: ModelRouteOption[];
  pendingError: string | null;
  turns: ConversationTurn[];
}) {
  const latestTurnId = turns.at(-1)?.id ?? null;

  return (
    <div aria-live="polite" className="space-y-10 pb-10">
      {turns.map((turn) => (
        <ConversationTurnTranscript
          isSubmitting={isSubmitting && turn.id === latestTurnId}
          key={turn.id}
          selectedModel={modelOptionForRoute(modelOptions, turn.modelRouteId)}
          turn={turn}
        />
      ))}
      {pendingError ? (
        <p className="rounded-[8px] border border-[#F3B5B5] bg-[#FFF5F5] px-4 py-3 text-[14px] text-[#A12626]" role="alert">
          {pendingError}
        </p>
      ) : null}
    </div>
  );
}

function ConversationTurnTranscript({
  isSubmitting,
  selectedModel,
  turn
}: {
  isSubmitting: boolean;
  selectedModel: ModelRouteOption;
  turn: ConversationTurn;
}) {
  const modelDeltaSummary = useMemo(() => summarizeModelDeltas(turn.runEvents), [turn.runEvents]);
  const eventEvidence = useMemo(
    () => turn.runEvents.map(summarizeWorkbenchEvent),
    [turn.runEvents]
  );

  return (
    <article>
      <p className="whitespace-pre-wrap text-[17px] font-medium leading-8 text-[#111111]">
        {turn.prompt}
      </p>

      <div className="mt-7 flex flex-wrap items-center gap-2 text-[14px] text-[#6F6F6F]">
        {turn.runResult ? (
          <>
            <span className="rounded-[7px] bg-[#EFEFEF] px-2 py-0.5 font-mono text-[13px] text-[#333333]">
              Run #{turn.runResult.runId}
            </span>
            <span>{turn.runResult.status}</span>
            <span className="rounded-[7px] bg-[#EFEFEF] px-2 py-0.5 font-mono text-[13px] text-[#333333]">
              {turn.runResult.traceId}
            </span>
          </>
        ) : (
          <span>{isSubmitting ? "Agent 正在运行" : "等待 Agent 输出"}</span>
        )}
        <span className="rounded-[7px] bg-[#EFEFEF] px-2 py-0.5 font-mono text-[13px] text-[#333333]">
          {selectedModel.routeId}
        </span>
      </div>

      {turn.runError ? (
        <p className="mt-4 rounded-[8px] border border-[#F3B5B5] bg-[#FFF5F5] px-4 py-3 text-[14px] text-[#A12626]" role="alert">
          {turn.runError}
        </p>
      ) : null}

      {turn.runResult ? (
        <section className="mt-4 rounded-[10px] bg-[#EFEFEF] px-4 py-4 font-mono text-[14px] leading-6 text-[#333333]">
          <p className="whitespace-pre-wrap">
            {turn.runResult.finalOutput || `Agent run ${turn.runResult.status}`}
          </p>
        </section>
      ) : null}

      {modelDeltaSummary ? (
        <section className="mt-4 rounded-[10px] border border-[#D7E7FF] bg-white px-4 py-4">
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
        <section className="mt-4 space-y-2">
          {eventEvidence.map((evidence) => (
            <article
              className="rounded-[8px] border border-[#EFEFEF] bg-white px-3 py-2"
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
    </article>
  );
}

function OutputRail({
  activeConversationId,
  eventEvidence,
  mcpTools,
  modelDeltaSummary,
  runEvents,
  runResult,
  selectedMcpToolCodes,
  selectedModel,
  selectedSkillCodes,
  skills,
  uploadedFiles,
  webSearchEnabled
}: {
  activeConversationId: string | null;
  eventEvidence: WorkbenchEventEvidence[];
  mcpTools: McpToolResp[];
  modelDeltaSummary: ModelDeltaSummary | null;
  runEvents: AgentRunEventResp[];
  runResult: AgentRunResp | null;
  selectedMcpToolCodes: string[];
  selectedModel: ModelRouteOption;
  selectedSkillCodes: string[];
  skills: CapabilityItemResp[];
  uploadedFiles: WorkbenchUploadedFile[];
  webSearchEnabled: boolean;
}) {
  const outputItems = outputRailItems(runResult, runEvents, modelDeltaSummary, eventEvidence);
  const sourceItems = sourceRailItems({
    mcpTools,
    selectedMcpToolCodes,
    selectedModel,
    selectedSkillCodes,
    skills,
    uploadedFiles,
    webSearchEnabled
  });

  return (
    <aside className="hidden h-screen w-[390px] shrink-0 overflow-y-auto border-l border-[#EFEFEF] px-6 py-6 xl:block">
      <div className="sticky top-6 rounded-[22px] border border-[#E8E8E8] bg-white px-5 py-5 shadow-[0_12px_36px_rgba(17,17,17,0.08)]">
        <section>
          <h2 className="mb-4 text-[15px] font-medium text-[#9A9A9A]">输出</h2>
          <div className="space-y-3">
            {outputItems.map((item) => (
              <div className="flex min-w-0 items-center gap-3 text-[15px] text-[#333333]" key={item.label}>
                <FileText aria-hidden="true" className="h-[18px] w-[18px] shrink-0 text-[#444444]" strokeWidth={1.9} />
                <div className="min-w-0">
                  <div className="truncate">{item.label}</div>
                  {item.detail ? <div className="truncate text-[12px] text-[#8A8A8A]">{item.detail}</div> : null}
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="mt-5 border-t border-[#EFEFEF] pt-5">
          <h2 className="mb-4 text-[15px] font-medium text-[#9A9A9A]">来源</h2>
          <div className="flex flex-wrap gap-2">
            {sourceItems.map((source, index) => (
              <span
                className="inline-flex h-7 max-w-full items-center gap-1.5 rounded-full border border-[#E5E5E5] px-2 text-[12px] text-[#555555]"
                key={`${source}-${index}`}
                title={source}
              >
                <Globe2 aria-hidden="true" className="h-3.5 w-3.5 shrink-0" strokeWidth={1.8} />
                <span className="min-w-0 truncate">{source}</span>
              </span>
            ))}
          </div>
          {activeConversationId ? (
            <p className="mt-3 truncate text-[12px] text-[#B0B0B0]">{activeConversationId}</p>
          ) : null}
        </section>
      </div>
    </aside>
  );
}

function ModelSelector({
  onSelect,
  options,
  selectedRouteId
}: {
  onSelect: (routeId: string) => void;
  options: ModelRouteOption[];
  selectedRouteId: string;
}) {
  const [open, setOpen] = useState(false);
  const selected = options.find((option) => option.routeId === selectedRouteId) ?? options[0];
  const showRoute = selected.label !== selected.routeId;

  return (
    <div className="relative">
      <button
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-label={`选择模型 ${selected.label}${showRoute ? ` ${selected.routeId}` : ""}`}
        className="inline-flex h-8 max-w-[250px] items-center gap-1 rounded-[9px] px-2 text-[15px] text-[#111111] hover:bg-[#F1F1F1]"
        onClick={() => setOpen((value) => !value)}
        type="button"
      >
        <span className="max-w-[120px] truncate">{selected.label}</span>
        {showRoute ? (
          <span className="hidden max-w-[130px] truncate text-[#8A8A8A] md:inline">{selected.routeId}</span>
        ) : null}
        <ChevronDown aria-hidden="true" className="h-[15px] w-[15px] shrink-0" strokeWidth={2} />
      </button>
      {open ? (
        <div
          aria-label="模型列表"
          className="absolute bottom-[calc(100%+8px)] right-0 z-30 min-w-[260px] rounded-[14px] border border-[#E5E5E5] bg-white p-1 shadow-[0_18px_44px_rgba(17,17,17,0.16)]"
          role="listbox"
        >
          {options.map((option) => {
            const selectedOption = option.routeId === selected.routeId;
            return (
              <button
                aria-label={`${option.label} ${option.routeId}`}
                aria-selected={selectedOption}
                className={[
                  "flex w-full items-center justify-between gap-3 rounded-[9px] px-3 py-2 text-left text-[14px]",
                  selectedOption ? "bg-[#F3F8FF] text-[#0A54A8]" : "text-[#333333] hover:bg-[#F6F6F6]"
                ].join(" ")}
                key={option.routeId}
                onClick={() => {
                  onSelect(option.routeId);
                  setOpen(false);
                }}
                role="option"
                type="button"
              >
                <span className="min-w-0">
                  <span className="block truncate font-medium">{option.label}</span>
                  <span className="block truncate text-[12px] text-[#8A8A8A]">{option.routeId}</span>
                </span>
                {selectedOption ? <Check aria-hidden="true" className="h-4 w-4 shrink-0" strokeWidth={2} /> : null}
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

function IconButton({
  children,
  label,
  onClick
}: {
  children: React.ReactNode;
  label: string;
  onClick?: () => void;
}) {
  return (
    <button aria-label={label} className="flex h-7 w-7 items-center justify-center rounded-[8px] text-[#8A8A8A] hover:bg-[#EDEDED] hover:text-[#111111]" onClick={onClick} type="button">
      {children}
    </button>
  );
}

function defaultProjects(): ProjectRecord[] {
  return [{ id: "project-default", name: CURRENT_PROJECT_NAME, turns: [] }];
}

function readStoredProjects(): ProjectRecord[] {
  if (typeof window === "undefined" || !window.localStorage) {
    return defaultProjects();
  }

  try {
    const parsed = JSON.parse(window.localStorage.getItem(PROJECT_STORAGE_KEY) ?? "null");
    if (!Array.isArray(parsed)) {
      return defaultProjects();
    }
    const projects = parsed
      .map(projectRecordFromValue)
      .filter((project): project is ProjectRecord => project !== null);
    return projects.length > 0 ? projects : defaultProjects();
  } catch {
    return defaultProjects();
  }
}

function writeStoredProjects(projects: ProjectRecord[]) {
  if (typeof window === "undefined" || !window.localStorage) {
    return;
  }
  window.localStorage.setItem(PROJECT_STORAGE_KEY, JSON.stringify(projects));
}

function readStoredActiveProjectId() {
  if (typeof window === "undefined" || !window.localStorage) {
    return "project-default";
  }
  return window.localStorage.getItem(ACTIVE_PROJECT_STORAGE_KEY) ?? "project-default";
}

function writeStoredActiveProjectId(projectId: string | null) {
  if (typeof window === "undefined" || !window.localStorage) {
    return;
  }
  if (projectId) {
    window.localStorage.setItem(ACTIVE_PROJECT_STORAGE_KEY, projectId);
  } else {
    window.localStorage.removeItem(ACTIVE_PROJECT_STORAGE_KEY);
  }
}

function projectRecordFromValue(value: unknown): ProjectRecord | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const record = value as Record<string, unknown>;
  const id = typeof record.id === "string" && record.id.trim() ? record.id : null;
  const name = typeof record.name === "string" && record.name.trim() ? record.name : null;
  if (!id || !name) {
    return null;
  }
  const turns = Array.isArray(record.turns)
    ? record.turns
        .map(conversationTurnFromValue)
        .filter((turn): turn is ConversationTurn => turn !== null)
    : [];

  return { id, name, turns };
}

function conversationTurnFromValue(value: unknown): ConversationTurn | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const record = value as Record<string, unknown>;
  const id = typeof record.id === "string" && record.id.trim() ? record.id : null;
  const prompt = typeof record.prompt === "string" ? record.prompt : null;
  if (!id || prompt === null) {
    return null;
  }

  return {
    id,
    prompt,
    modelRouteId:
      typeof record.modelRouteId === "string" && record.modelRouteId.trim()
        ? record.modelRouteId
        : DEFAULT_MODEL_ROUTE_ID,
    runResult: objectOrNull<AgentRunResp>(record.runResult),
    runEvents: Array.isArray(record.runEvents)
      ? (record.runEvents.filter((event) => event && typeof event === "object") as AgentRunEventResp[])
      : [],
    runError: typeof record.runError === "string" ? record.runError : null
  };
}

function objectOrNull<T>(value: unknown): T | null {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as T) : null;
}

function nextProjectNumber(projects: ProjectRecord[]) {
  const used = new Set(
    projects
      .map((project) => /^新项目 (\d+)$/.exec(project.name)?.[1])
      .filter((value): value is string => Boolean(value))
      .map((value) => Number(value))
  );
  let next = 1;
  while (used.has(next)) {
    next += 1;
  }
  return next;
}

function updateProjectTurn(
  project: ProjectRecord,
  turnId: string,
  patch: Partial<ConversationTurn>
): ProjectRecord {
  return {
    ...project,
    turns: project.turns.map((turn) => (turn.id === turnId ? { ...turn, ...patch } : turn))
  };
}

function conversationSummariesForProject(project: ProjectRecord | undefined): ConversationSummary[] {
  return (project?.turns ?? [])
    .slice()
    .reverse()
    .slice(0, 12)
    .map((turn) => ({
      id: turn.id,
      title: conversationTitleFromPrompt(turn.prompt),
      age: "刚刚",
      status: turn.runError ? "failed" : turn.runResult?.status ?? "running"
    }));
}

function modelOptionForRoute(options: ModelRouteOption[], routeId: string): ModelRouteOption {
  return options.find((option) => option.routeId === routeId) ?? { routeId, label: labelForRoute(routeId) };
}

function toggleString(values: string[], value: string) {
  return values.includes(value)
    ? values.filter((current) => current !== value)
    : [...values, value];
}

function configuredModelRouteOptions(): ModelRouteOption[] {
  const parsed = (process.env.NEXT_PUBLIC_AGENT_MODEL_ROUTE_OPTIONS ?? "")
    .split(",")
    .map(parseModelRouteOption)
    .filter((option): option is ModelRouteOption => option !== null);
  const configuredRouteId = (process.env.NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID ?? "").trim();
  const seed =
    configuredRouteId && !parsed.some((option) => option.routeId === configuredRouteId)
      ? [{ routeId: configuredRouteId, label: labelForRoute(configuredRouteId) }, ...parsed]
      : parsed;
  const unique = seed.reduce<ModelRouteOption[]>((items, option) => {
    if (items.some((item) => item.routeId === option.routeId)) {
      return items;
    }
    return [...items, option];
  }, []);

  if (unique.length === 0) {
    return [{ routeId: DEFAULT_MODEL_ROUTE_ID, label: DEFAULT_MODEL_ROUTE_ID }];
  }

  if (!configuredRouteId) {
    return unique;
  }

  const selected = unique.find((option) => option.routeId === configuredRouteId);
  return selected ? [selected, ...unique.filter((option) => option.routeId !== configuredRouteId)] : unique;
}

function parseModelRouteOption(value: string): ModelRouteOption | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  const [routeIdPart, ...labelParts] = trimmed.split(":");
  const routeId = routeIdPart.trim();
  if (!routeId) {
    return null;
  }

  return {
    routeId,
    label: labelParts.join(":").trim() || labelForRoute(routeId)
  };
}

function labelForRoute(routeId: string) {
  return routeId;
}

function conversationTitleFromPrompt(prompt: string) {
  const compact = prompt.replace(/\s+/g, " ").trim();
  return compact.length > 26 ? `${compact.slice(0, 26)}...` : compact;
}

function outputRailItems(
  runResult: AgentRunResp | null,
  runEvents: AgentRunEventResp[],
  modelDeltaSummary: ModelDeltaSummary | null,
  eventEvidence: WorkbenchEventEvidence[]
) {
  const items: Array<{ label: string; detail?: string }> = [];
  if (runResult) {
    items.push({
      label: `Run #${runResult.runId}`,
      detail: runResult.status
    });
  }
  if (modelDeltaSummary) {
    items.push({
      label: "Live model output",
      detail: modelDeltaSummary.routeId ?? modelDeltaSummary.model
    });
  }
  if (runEvents.length > 0) {
    items.push({
      label: `${runEvents.length} run events`,
      detail: eventEvidence.map((event) => event.title).slice(0, 3).join(", ")
    });
  }

  return items.length > 0 ? items : [{ label: "等待 Agent 输出" }];
}

function sourceRailItems({
  mcpTools,
  selectedMcpToolCodes,
  selectedModel,
  selectedSkillCodes,
  skills,
  uploadedFiles,
  webSearchEnabled
}: {
  mcpTools: McpToolResp[];
  selectedMcpToolCodes: string[];
  selectedModel: ModelRouteOption;
  selectedSkillCodes: string[];
  skills: CapabilityItemResp[];
  uploadedFiles: WorkbenchUploadedFile[];
  webSearchEnabled: boolean;
}) {
  const skillSources = selectedSkillCodes.map(
    (code) => skills.find((skill) => skill.code === code)?.name || code
  );
  const mcpSources = selectedMcpToolCodes.map(
    (code) => mcpTools.find((tool) => tool.toolCode === code)?.toolName || code
  );
  const fileSources = uploadedFiles.map((file) => file.name);

  return [
    ...fileSources,
    ...skillSources,
    ...mcpSources,
    ...(webSearchEnabled ? ["Web search"] : []),
    selectedModel.label,
    "Backend"
  ];
}
