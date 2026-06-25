"use client";

import { useMemo, useState } from "react";
import {
  Activity,
  ArrowUpRight,
  BookOpen,
  Boxes,
  Brain,
  Check,
  ChevronDown,
  Database,
  FileText,
  FlaskConical,
  GitBranch,
  Globe2,
  History,
  Newspaper,
  Radar,
  Search,
  Sparkles,
  Users
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { listAgentRunEvents } from "@/api/agent";
import { configuredModelRouteOptions, createResearchRadarRun } from "@/api/research";
import { createResearchRadarSourceScan } from "@/api/source-scan";
import { summarizeModelDeltas, summarizeResearchEvent } from "@/lib/agent-events";
import { buildResearchGraph, nodeDetailsFor } from "@/lib/research-graph";
import { parseResearchReport } from "@/lib/research-report";
import { ResearchMap } from "@/components/research-map";
import type { ModelDeltaSummary, ResearchEventEvidence } from "@/lib/agent-events";
import type { AgentRunEventResp } from "@/types/agent";
import type {
  ModelRouteOption,
  ParsedResearchReport,
  ResearchFilter,
  ResearchRanking,
  ResearchScan,
  ResearchGraph,
  ResearchGraphNode,
  ResearchSource,
  ResearchSourceMetric,
  ResearchSourceResult,
  ResearchSourceStatus
} from "@/types/research";

const DEFAULT_FILTERS: ResearchFilter[] = ["papers", "projects", "datasets", "benchmarks"];

const FILTERS: Array<{
  code: ResearchFilter;
  label: string;
  icon: LucideIcon;
}> = [
  { code: "papers", label: "Papers", icon: FileText },
  { code: "projects", label: "Projects", icon: GitBranch },
  { code: "datasets", label: "Datasets", icon: Database },
  { code: "benchmarks", label: "Benchmarks", icon: Boxes },
  { code: "news", label: "News", icon: Newspaper },
  { code: "community", label: "Community", icon: Users }
];

const RANKINGS: Array<{ code: ResearchRanking; label: string }> = [
  { code: "balanced", label: "Balanced" },
  { code: "importance", label: "Importance" },
  { code: "recency", label: "Recency" },
  { code: "beginner", label: "Beginner" }
];

const SECTION_ICONS: Record<string, LucideIcon> = {
  "research-overview": Brain,
  "active-topics": Activity,
  "key-authors-and-institutions": Users,
  "representative-work": BookOpen,
  "reading-route": ArrowUpRight,
  "research-openings": Sparkles,
  "experiment-plans": FlaskConical,
  "sources-and-caveats": Globe2,
  raw: FileText
};

const SOURCE_LABELS: Record<ResearchSource, string> = {
  arxiv: "arXiv",
  github: "GitHub",
  huggingface_models: "HuggingFace Models",
  huggingface_datasets: "HuggingFace Datasets",
  paperswithcode: "PapersWithCode",
  leaderboards: "Leaderboards"
};

const SOURCE_STATUS_LABELS: Record<ResearchSourceStatus, string> = {
  succeeded: "ready",
  degraded: "limited",
  failed: "failed"
};

export function ResearchRadarApp() {
  const modelOptions = useMemo(() => configuredModelRouteOptions(), []);
  const [topic, setTopic] = useState("");
  const [filters, setFilters] = useState<ResearchFilter[]>(DEFAULT_FILTERS);
  const [ranking, setRanking] = useState<ResearchRanking>("balanced");
  const [selectedRouteId, setSelectedRouteId] = useState(modelOptions[0]?.routeId ?? "runtime.llm");
  const [scans, setScans] = useState<ResearchScan[]>([]);
  const [activeScanId, setActiveScanId] = useState<string | null>(null);
  const [selectedGraphNodeId, setSelectedGraphNodeId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [composerError, setComposerError] = useState<string | null>(null);
  const activeScan = scans.find((scan) => scan.id === activeScanId) ?? scans[0] ?? null;
  const parsedReport = useMemo(
    () => parseResearchReport(activeScan?.runResult?.finalOutput ?? ""),
    [activeScan?.runResult?.finalOutput]
  );
  const researchGraph = useMemo(
    () =>
      activeScan && activeScan.sourceScan?.status !== "failed"
        ? buildResearchGraph({
            topic: activeScan.topic,
            sourceScan: activeScan.sourceScan,
            parsedReport,
            finalOutput: activeScan.runResult?.finalOutput ?? ""
          })
        : null,
    [activeScan, parsedReport]
  );
  const selectedGraphNode = useMemo(
    () => (researchGraph && selectedGraphNodeId ? nodeDetailsFor(researchGraph, selectedGraphNodeId) : null),
    [researchGraph, selectedGraphNodeId]
  );
  const eventEvidence = useMemo(
    () =>
      (activeScan?.runEvents ?? [])
        .map(summarizeResearchEvent)
        .filter((event) => event.kind !== "model"),
    [activeScan?.runEvents]
  );
  const modelDeltaSummary = useMemo(
    () => summarizeModelDeltas(activeScan?.runEvents ?? []),
    [activeScan?.runEvents]
  );

  async function handleSubmit() {
    const normalizedTopic = topic.trim();
    if (!normalizedTopic || isSubmitting) {
      setComposerError("请输入研究主题");
      return;
    }

    const scanId = `scan-${Date.now()}`;
    const nextScan: ResearchScan = {
      id: scanId,
      topic: normalizedTopic,
      filters,
      ranking,
      routeId: selectedRouteId,
      runResult: null,
      runEvents: [],
      runError: null,
      sourceScan: null,
      createdAt: Date.now()
    };

    setScans((items) => [nextScan, ...items]);
    setActiveScanId(scanId);
    setSelectedGraphNodeId(null);
    setComposerError(null);
    setIsSubmitting(true);
    let hasUsableSourceScan = false;

    try {
      const sourceScan = await createResearchRadarSourceScan({
        topic: normalizedTopic,
        filters,
        ranking
      });
      updateScan(scanId, { sourceScan });
      hasUsableSourceScan = sourceScan.status !== "failed";

      if (sourceScan.status === "failed") {
        updateScan(scanId, {
          runError: sourceScan.warnings.join("\n") || "研究来源扫描失败"
        });
        return;
      }

      const runResult = await createResearchRadarRun({
        topic: normalizedTopic,
        filters,
        ranking,
        routeId: selectedRouteId,
        sourceScan
      });
      let runEvents: AgentRunEventResp[] = [];
      try {
        const eventPage = await listAgentRunEvents(runResult.runId, { page: 1, size: 100 });
        runEvents = eventPage.list;
      } catch {
        runEvents = [];
      }
      updateScan(scanId, { runResult, runEvents });
    } catch (error) {
      updateScan(scanId, {
        runError: hasUsableSourceScan
          ? "model analysis unavailable"
          : error instanceof Error
            ? error.message
            : "雷达扫描失败"
      });
    } finally {
      setIsSubmitting(false);
    }
  }

  function updateScan(scanId: string, patch: Partial<ResearchScan>) {
    setScans((items) => items.map((scan) => (scan.id === scanId ? { ...scan, ...patch } : scan)));
  }

  function toggleFilter(code: ResearchFilter) {
    setFilters((items) =>
      items.includes(code) ? items.filter((item) => item !== code) : [...items, code]
    );
  }

  return (
    <main className="min-h-screen bg-[#F6F8F5] text-[#171717]">
      <div className="grid min-h-screen grid-cols-1 xl:grid-cols-[286px_minmax(0,1fr)_382px]">
        <ScanSidebar
          activeScanId={activeScan?.id ?? null}
          onScanSelect={setActiveScanId}
          scans={scans}
        />
        <section className="min-w-0 border-x border-[#DFE5DF] bg-white">
          <Header
            isSubmitting={isSubmitting}
            modelOptions={modelOptions}
            onRouteSelect={setSelectedRouteId}
            selectedRouteId={selectedRouteId}
          />
          <div className="mx-auto flex w-full max-w-[1120px] flex-col gap-5 px-5 py-5 lg:px-7">
            <TopicComposer
              composerError={composerError}
              filters={filters}
              isSubmitting={isSubmitting}
              onFilterToggle={toggleFilter}
              onRankingSelect={setRanking}
              onSubmit={handleSubmit}
              onTopicChange={setTopic}
              ranking={ranking}
              topic={topic}
            />
            <ReportWorkspace
              activeScan={activeScan}
              isSubmitting={isSubmitting}
              onGraphNodeSelect={setSelectedGraphNodeId}
              parsedReport={parsedReport}
              researchGraph={researchGraph}
              selectedGraphNodeId={selectedGraphNodeId}
            />
          </div>
        </section>
        <EvidenceRail
          activeScan={activeScan}
          eventEvidence={eventEvidence}
          modelDeltaSummary={modelDeltaSummary}
          researchGraph={researchGraph}
          selectedGraphNode={selectedGraphNode}
        />
      </div>
    </main>
  );
}

function Header({
  isSubmitting,
  modelOptions,
  onRouteSelect,
  selectedRouteId
}: {
  isSubmitting: boolean;
  modelOptions: ModelRouteOption[];
  onRouteSelect: (routeId: string) => void;
  selectedRouteId: string;
}) {
  return (
    <header className="flex min-h-[74px] items-center justify-between gap-4 border-b border-[#E5EAE5] px-5 py-4 lg:px-7">
      <div className="flex min-w-0 items-center gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-[8px] bg-[#0E6B5F] text-white">
          <Radar aria-hidden="true" className="h-5 w-5" strokeWidth={2} />
        </div>
        <div className="min-w-0">
          <h1 className="truncate text-[21px] font-semibold leading-7 text-[#111111]">
            Research Radar
          </h1>
          <div className="flex min-w-0 items-center gap-2 text-[12px] text-[#64706A]">
            <span className={["h-2 w-2 rounded-full", isSubmitting ? "bg-[#D97706]" : "bg-[#0E9F6E]"].join(" ")} />
            <span className="truncate">{isSubmitting ? "scanning" : "ready"}</span>
          </div>
        </div>
      </div>
      <ModelSelector
        onSelect={onRouteSelect}
        options={modelOptions}
        selectedRouteId={selectedRouteId}
      />
    </header>
  );
}

function ScanSidebar({
  activeScanId,
  onScanSelect,
  scans
}: {
  activeScanId: string | null;
  onScanSelect: (scanId: string) => void;
  scans: ResearchScan[];
}) {
  return (
    <aside className="hidden min-h-screen bg-[#EEF3ED] px-4 py-5 xl:block">
      <div className="mb-4 flex items-center gap-2 text-[13px] font-medium text-[#66736B]">
        <History aria-hidden="true" className="h-4 w-4" strokeWidth={1.9} />
        Scans
      </div>
      <div className="space-y-2">
        {scans.length === 0 ? (
          <div className="rounded-[8px] border border-dashed border-[#D3DDD4] bg-white/60 px-3 py-3 text-[13px] text-[#7A857E]">
            No scans
          </div>
        ) : (
          scans.map((scan) => (
            <button
              className={[
                "w-full rounded-[8px] border px-3 py-3 text-left transition-colors",
                scan.id === activeScanId
                  ? "border-[#0E6B5F] bg-white text-[#111111] shadow-sm"
                  : "border-[#DDE5DD] bg-white/70 text-[#536058] hover:bg-white"
              ].join(" ")}
              key={scan.id}
              onClick={() => onScanSelect(scan.id)}
              type="button"
            >
              <span className="block truncate text-[14px] font-medium">{scan.topic}</span>
              <span className="mt-1 block truncate text-[12px] text-[#7A857E]">
                {scan.runResult ? `#${scan.runResult.runId}` : scan.runError ? "failed" : "pending"}
              </span>
            </button>
          ))
        )}
      </div>
    </aside>
  );
}

function TopicComposer({
  composerError,
  filters,
  isSubmitting,
  onFilterToggle,
  onRankingSelect,
  onSubmit,
  onTopicChange,
  ranking,
  topic
}: {
  composerError: string | null;
  filters: ResearchFilter[];
  isSubmitting: boolean;
  onFilterToggle: (code: ResearchFilter) => void;
  onRankingSelect: (ranking: ResearchRanking) => void;
  onSubmit: () => void;
  onTopicChange: (topic: string) => void;
  ranking: ResearchRanking;
  topic: string;
}) {
  return (
    <section className="rounded-[8px] border border-[#DEE6DE] bg-[#FBFCFA] p-4 shadow-[0_12px_28px_rgba(34,45,38,0.06)]">
      <label className="block text-[13px] font-medium text-[#59665F]" htmlFor="research-topic">
        研究主题
      </label>
      <div className="mt-2 flex flex-col gap-3 lg:flex-row">
        <div className="relative min-w-0 flex-1">
          <Search aria-hidden="true" className="pointer-events-none absolute left-3 top-3 h-5 w-5 text-[#7A857E]" strokeWidth={1.9} />
          <textarea
            className="min-h-[54px] w-full resize-none rounded-[8px] border border-[#D7E0D7] bg-white py-3 pl-10 pr-3 text-[16px] leading-6 text-[#111111] outline-none transition focus:border-[#0E6B5F] focus:ring-2 focus:ring-[#BFE5DC]"
            id="research-topic"
            onChange={(event) => onTopicChange(event.target.value)}
            placeholder="例如：AI coding agents"
            value={topic}
          />
        </div>
        <button
          className="inline-flex h-[54px] shrink-0 items-center justify-center gap-2 rounded-[8px] bg-[#0E6B5F] px-5 text-[15px] font-semibold text-white transition hover:bg-[#0A5C52] disabled:cursor-not-allowed disabled:bg-[#93A39D]"
          disabled={isSubmitting}
          onClick={onSubmit}
          type="button"
        >
          <Radar aria-hidden="true" className="h-4 w-4" strokeWidth={2} />
          启动雷达扫描
        </button>
      </div>
      {composerError ? (
        <p className="mt-2 rounded-[8px] border border-[#F4C7C3] bg-[#FFF7F5] px-3 py-2 text-[13px] text-[#A33A2D]" role="alert">
          {composerError}
        </p>
      ) : null}
      <div className="mt-4 flex flex-col gap-3 2xl:flex-row 2xl:items-center 2xl:justify-between">
        <FilterDock filters={filters} onToggle={onFilterToggle} />
        <RankingControl onSelect={onRankingSelect} ranking={ranking} />
      </div>
    </section>
  );
}

function FilterDock({
  filters,
  onToggle
}: {
  filters: ResearchFilter[];
  onToggle: (code: ResearchFilter) => void;
}) {
  return (
    <div className="flex flex-wrap gap-2">
      {FILTERS.map((filter) => {
        const selected = filters.includes(filter.code);
        const Icon = filter.icon;
        return (
          <button
            aria-pressed={selected}
            className={[
              "inline-flex h-9 items-center gap-1.5 rounded-[8px] border px-2.5 text-[13px] font-medium transition",
              selected
                ? "border-[#0E6B5F] bg-[#E9F7F3] text-[#0B5D53]"
                : "border-[#DDE5DD] bg-white text-[#5F6B64] hover:border-[#B7C6BA]"
            ].join(" ")}
            key={filter.code}
            onClick={() => onToggle(filter.code)}
            type="button"
          >
            <Icon aria-hidden={true} className="h-3.5 w-3.5" strokeWidth={1.9} />
            {filter.label}
          </button>
        );
      })}
    </div>
  );
}

function RankingControl({
  onSelect,
  ranking
}: {
  onSelect: (ranking: ResearchRanking) => void;
  ranking: ResearchRanking;
}) {
  return (
    <div className="flex flex-wrap gap-1 rounded-[8px] border border-[#DDE5DD] bg-white p-1">
      {RANKINGS.map((option) => (
        <button
          aria-pressed={ranking === option.code}
          className={[
            "h-8 rounded-[7px] px-3 text-[13px] font-medium transition",
            ranking === option.code
              ? "bg-[#17251F] text-white"
              : "text-[#66736B] hover:bg-[#F0F4F0] hover:text-[#1F2923]"
          ].join(" ")}
          key={option.code}
          onClick={() => onSelect(option.code)}
          type="button"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function ReportWorkspace({
  activeScan,
  isSubmitting,
  onGraphNodeSelect,
  parsedReport,
  researchGraph,
  selectedGraphNodeId
}: {
  activeScan: ResearchScan | null;
  isSubmitting: boolean;
  onGraphNodeSelect: (nodeId: string) => void;
  parsedReport: ParsedResearchReport;
  researchGraph: ResearchGraph | null;
  selectedGraphNodeId: string | null;
}) {
  if (!activeScan) {
    return (
      <section className="grid gap-4 lg:grid-cols-3">
        {[
          ["Signal", "topic velocity", "#0E6B5F"],
          ["People", "authors and labs", "#2563EB"],
          ["Experiments", "next probes", "#B45309"]
        ].map(([title, value, color]) => (
          <div className="rounded-[8px] border border-[#E0E7E0] bg-white p-5" key={title}>
            <div className="mb-5 h-1.5 w-12 rounded-full" style={{ backgroundColor: color }} />
            <h2 className="text-[15px] font-semibold text-[#1A211D]">{title}</h2>
            <p className="mt-2 text-[13px] leading-5 text-[#6C776F]">{value}</p>
          </div>
        ))}
      </section>
    );
  }

  return (
    <section className="space-y-5">
      <div className="rounded-[8px] border border-[#DEE6DE] bg-white p-5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="min-w-0">
            <h2 className="truncate text-[20px] font-semibold leading-7 text-[#111111]">
              {activeScan.topic}
            </h2>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-[12px] text-[#6B776F]">
              <span className="rounded-[7px] bg-[#EEF3ED] px-2 py-1">{rankingLabel(activeScan.ranking)}</span>
              {activeScan.runResult ? (
                <span className="rounded-[7px] bg-[#EEF3ED] px-2 py-1">Run #{activeScan.runResult.runId}</span>
              ) : null}
              <span className="rounded-[7px] bg-[#EEF3ED] px-2 py-1">
                {activeScan.runResult?.status ?? (activeScan.runError ? "failed" : isSubmitting ? "running" : "pending")}
              </span>
            </div>
          </div>
          <div className="grid grid-cols-3 gap-2 text-center">
            <Metric
              value={String(activeScan.sourceScan?.sources.length ?? activeScan.filters.length)}
              label="sources"
            />
            <Metric value={(activeScan.runEvents ?? []).length.toString()} label="events" />
            <Metric value={parsedReport.structured ? "8" : "1"} label="sections" />
          </div>
        </div>
      </div>

      {researchGraph ? (
        <ResearchMap
          graph={researchGraph}
          onNodeSelect={onGraphNodeSelect}
          selectedNodeId={selectedGraphNodeId}
        />
      ) : null}

      {activeScan.runError ? (
        <p className="rounded-[8px] border border-[#F4C7C3] bg-[#FFF7F5] px-4 py-3 text-[14px] text-[#A33A2D]" role="alert">
          {activeScan.runError}
        </p>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-2">
        {parsedReport.sections.map((section) => {
          const Icon = SECTION_ICONS[section.id] ?? FileText;
          return (
            <article
              className="rounded-[8px] border border-[#E0E7E0] bg-white p-5 shadow-[0_10px_24px_rgba(34,45,38,0.05)]"
              key={section.id}
            >
              <div className="mb-3 flex items-center gap-2">
                <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[8px] bg-[#EDF7F4] text-[#0E6B5F]">
                  <Icon aria-hidden={true} className="h-4 w-4" strokeWidth={1.9} />
                </span>
                <h3 className="text-[15px] font-semibold text-[#17251F]">{section.title}</h3>
              </div>
              <p className="whitespace-pre-wrap text-[14px] leading-6 text-[#3D4841]">
                {section.content || "Pending model output"}
              </p>
            </article>
          );
        })}
      </div>

      {activeScan.sourceScan ? <EvidenceDrawer sources={activeScan.sourceScan.sources} /> : null}
    </section>
  );
}

function EvidenceDrawer({ sources }: { sources: ResearchSourceResult[] }) {
  const [open, setOpen] = useState(false);

  return (
    <section className="rounded-[8px] border border-[#DEE6DE] bg-white p-4">
      <button
        aria-label="Evidence Drawer"
        aria-expanded={open}
        className="flex w-full items-center justify-between gap-3 text-left"
        onClick={() => setOpen((value) => !value)}
        type="button"
      >
        <span>
          <span className="block text-[15px] font-semibold text-[#17251F]">Evidence Drawer</span>
          <span className="block text-[12px] text-[#6B776F]">
            Raw API results and source warnings
          </span>
        </span>
        <ChevronDown
          aria-hidden="true"
          className={["h-4 w-4 shrink-0 transition", open ? "rotate-180" : ""].join(" ")}
          strokeWidth={1.9}
        />
      </button>
      {open ? (
        <div className="mt-4">
          <SourceResults sources={sources} />
        </div>
      ) : null}
    </section>
  );
}

function SourceResults({ sources }: { sources: ResearchSourceResult[] }) {
  if (sources.length === 0) {
    return (
      <section className="rounded-[8px] border border-[#DEE6DE] bg-white p-5">
        <h3 className="text-[15px] font-semibold text-[#17251F]">Source Results</h3>
        <p className="mt-3 rounded-[8px] border border-dashed border-[#D7E0D7] bg-[#FBFCFA] px-3 py-3 text-[13px] text-[#7A857E]">
          Waiting for source evidence
        </p>
      </section>
    );
  }

  return (
    <section className="rounded-[8px] border border-[#DEE6DE] bg-white p-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h3 className="text-[15px] font-semibold text-[#17251F]">Source Results</h3>
          <p className="mt-1 text-[12px] text-[#6B776F]">
            API evidence collected before the model report
          </p>
        </div>
        <span className="rounded-[7px] bg-[#EEF3ED] px-2 py-1 text-[12px] text-[#66736B]">
          {sources.reduce((total, source) => total + source.items.length, 0)} items
        </span>
      </div>

      <div className="mt-4 divide-y divide-[#E8EEE8]">
        {sources.map((source) => (
          <SourceResultRow key={source.source} source={source} />
        ))}
      </div>
    </section>
  );
}

function SourceResultRow({ source }: { source: ResearchSourceResult }) {
  const statusTone = sourceStatusTone(source.status);

  return (
    <article className="py-4 first:pt-0 last:pb-0">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <span className={["h-2 w-2 shrink-0 rounded-full", statusTone.dot].join(" ")} />
            <h4 className="truncate text-[14px] font-semibold text-[#17251F]">
              {SOURCE_LABELS[source.source] ?? source.source}
            </h4>
          </div>
          {source.warning ? (
            <p className="mt-1 text-[12px] leading-5 text-[#A16016]">{source.warning}</p>
          ) : null}
        </div>
        <div className="flex items-center gap-2 text-[12px]">
          <span className={["rounded-[7px] px-2 py-1", statusTone.badge].join(" ")}>
            {SOURCE_STATUS_LABELS[source.status] ?? source.status}
          </span>
          <span className="rounded-[7px] bg-[#EEF3ED] px-2 py-1 text-[#66736B]">
            {source.items.length} items
          </span>
        </div>
      </div>

      {source.items.length > 0 ? (
        <div className="mt-3 grid gap-2">
          {source.items.slice(0, 3).map((item) => (
            <a
              className="group block rounded-[8px] border border-[#E5ECE5] bg-[#FBFCFA] px-3 py-3 transition hover:border-[#B8CCC4] hover:bg-white"
              href={item.url ?? undefined}
              key={item.id}
              rel="noreferrer"
              target="_blank"
            >
              <div className="flex min-w-0 items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex min-w-0 flex-wrap items-center gap-2">
                    <span className="rounded-[7px] bg-[#EDF7F4] px-2 py-0.5 text-[11px] font-medium text-[#0B5D53]">
                      {item.kind}
                    </span>
                    <h5 className="min-w-0 break-words text-[13px] font-semibold leading-5 text-[#17251F]">
                      {item.title}
                    </h5>
                  </div>
                  <p className="mt-1 line-clamp-2 text-[12px] leading-5 text-[#5B675F]">
                    {item.summary || item.authors.slice(0, 3).join(", ") || item.organization || "No summary"}
                  </p>
                </div>
                {item.url ? (
                  <ArrowUpRight
                    aria-hidden="true"
                    className="mt-0.5 h-4 w-4 shrink-0 text-[#8A968F] transition group-hover:text-[#0E6B5F]"
                    strokeWidth={1.9}
                  />
                ) : null}
              </div>
              {item.metrics.length > 0 ? (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {item.metrics.slice(0, 3).map((metric) => (
                    <SourceMetric key={`${item.id}-${metric.label}`} metric={metric} />
                  ))}
                </div>
              ) : null}
            </a>
          ))}
        </div>
      ) : (
        <p className="mt-3 rounded-[8px] border border-dashed border-[#D7E0D7] bg-[#FBFCFA] px-3 py-2 text-[12px] text-[#7A857E]">
          No items returned
        </p>
      )}
    </article>
  );
}

function SourceMetric({ metric }: { metric: ResearchSourceMetric }) {
  return (
    <span className="rounded-[7px] bg-[#EEF3ED] px-2 py-0.5 text-[11px] text-[#66736B]">
      {metric.label} {metric.value}
    </span>
  );
}

function sourceStatusTone(status: ResearchSourceStatus) {
  if (status === "succeeded") {
    return {
      dot: "bg-[#0E9F6E]",
      badge: "bg-[#E9F7F3] text-[#0B5D53]"
    };
  }
  if (status === "degraded") {
    return {
      dot: "bg-[#D97706]",
      badge: "bg-[#FFF5E6] text-[#A16016]"
    };
  }
  return {
    dot: "bg-[#D64B3C]",
    badge: "bg-[#FFF0EE] text-[#A33A2D]"
  };
}

function EvidenceRail({
  activeScan,
  eventEvidence,
  modelDeltaSummary,
  researchGraph,
  selectedGraphNode
}: {
  activeScan: ResearchScan | null;
  eventEvidence: ResearchEventEvidence[];
  modelDeltaSummary: ModelDeltaSummary | null;
  researchGraph: ResearchGraph | null;
  selectedGraphNode: ResearchGraphNode | null;
}) {
  return (
    <aside className="hidden min-h-screen bg-[#FAFBF8] px-5 py-5 xl:block">
      <div className="rounded-[8px] border border-[#E0E7E0] bg-white p-4 shadow-[0_10px_26px_rgba(34,45,38,0.06)]">
        <h2 className="mb-4 flex items-center gap-2 text-[14px] font-semibold text-[#59665F]">
          <Globe2 aria-hidden="true" className="h-4 w-4" strokeWidth={1.9} />
          Evidence
        </h2>
        {selectedGraphNode ? (
          <section className="mb-4 rounded-[8px] border border-[#D7E7FF] bg-[#F8FBFF] p-3">
            <h2 className="text-[14px] font-semibold text-[#1D2B39]">Node Inspector</h2>
            <div className="mt-2 flex flex-wrap gap-1.5">
              <span className="rounded-[7px] bg-white px-2 py-0.5 text-[11px] text-[#53687F]">
                kind {selectedGraphNode.kind}
              </span>
              <span className="rounded-[7px] bg-white px-2 py-0.5 text-[11px] text-[#53687F]">
                importance {selectedGraphNode.importance.toFixed(2)}
              </span>
            </div>
            <h3 className="mt-3 text-[15px] font-semibold text-[#17251F]">
              {selectedGraphNode.title}
            </h3>
            <p className="mt-2 whitespace-pre-wrap text-[13px] leading-5 text-[#3D4841]">
              {selectedGraphNode.summary || "No node summary available."}
            </p>
            {selectedGraphNode.tags.length > 0 ? (
              <div className="mt-3 flex flex-wrap gap-1.5">
                {selectedGraphNode.tags.slice(0, 6).map((tag) => (
                  <span className="rounded-[7px] bg-white px-2 py-0.5 text-[11px] text-[#53687F]" key={tag}>
                    {tag}
                  </span>
                ))}
              </div>
            ) : null}
          </section>
        ) : researchGraph ? (
          <p className="mb-4 rounded-[8px] border border-dashed border-[#D7E0D7] bg-[#FBFCFA] px-3 py-3 text-[13px] text-[#7A857E]">
            Select a node in the research map
          </p>
        ) : activeScan?.runResult ? (
          <div className="mb-4 grid grid-cols-2 gap-2">
            <EvidenceMeta label="run" value={`#${activeScan.runResult.runId}`} />
            <EvidenceMeta label="status" value={activeScan.runResult.status} />
            <EvidenceMeta className="col-span-2" label="trace" value={activeScan.runResult.traceId} />
          </div>
        ) : (
          <p className="rounded-[8px] border border-dashed border-[#D7E0D7] bg-[#FBFCFA] px-3 py-3 text-[13px] text-[#7A857E]">
            Waiting for scan
          </p>
        )}

        {modelDeltaSummary ? (
          <section className="mb-4 rounded-[8px] border border-[#D7E7FF] bg-[#F8FBFF] p-3">
            <div className="mb-2 flex items-center justify-between gap-2 text-[12px] text-[#53687F]">
              <span className="font-semibold text-[#1D2B39]">Live model output</span>
              <span>{modelDeltaSummary.chunkCount} chunks</span>
            </div>
            <p className="whitespace-pre-wrap text-[13px] leading-5 text-[#263747]">
              {modelDeltaSummary.text}
            </p>
          </section>
        ) : null}

        <div className="space-y-2">
          {eventEvidence.map((event) => (
            <article className="rounded-[8px] border border-[#E6ECE6] bg-[#FCFDFC] px-3 py-2" key={`${event.sequenceNo}-${event.title}`}>
              <div className="flex items-center justify-between gap-2">
                <h3 className="truncate text-[13px] font-semibold text-[#17251F]">{event.title}</h3>
                <span className="rounded-[7px] bg-[#EEF3ED] px-2 py-0.5 text-[11px] text-[#66736B]">
                  {event.kind}
                </span>
              </div>
              <p className="mt-1 whitespace-pre-wrap text-[12px] leading-5 text-[#5B675F]">
                {event.text}
              </p>
            </article>
          ))}
        </div>
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
  const selected = options.find((option) => option.routeId === selectedRouteId) ?? options[0] ?? {
    routeId: selectedRouteId,
    label: selectedRouteId
  };

  return (
    <div className="relative">
      <button
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-label={`选择模型 ${selected.label}`}
        className="inline-flex h-9 max-w-[260px] items-center gap-2 rounded-[8px] border border-[#D7E0D7] bg-white px-3 text-[13px] font-medium text-[#233029] hover:bg-[#F5F8F5]"
        onClick={() => setOpen((value) => !value)}
        type="button"
      >
        <span className="truncate">{selected.label}</span>
        <ChevronDown aria-hidden="true" className="h-4 w-4 shrink-0" strokeWidth={1.9} />
      </button>
      {open ? (
        <div
          aria-label="模型列表"
          className="absolute right-0 z-30 mt-2 min-w-[260px] rounded-[8px] border border-[#D7E0D7] bg-white p-1 shadow-[0_18px_42px_rgba(34,45,38,0.15)]"
          role="listbox"
        >
          {options.map((option) => {
            const isSelected = option.routeId === selected.routeId;
            return (
              <button
                aria-selected={isSelected}
                className={[
                  "flex w-full items-center justify-between gap-3 rounded-[7px] px-3 py-2 text-left text-[13px]",
                  isSelected ? "bg-[#E9F7F3] text-[#0B5D53]" : "text-[#3D4841] hover:bg-[#F4F7F4]"
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
                  <span className="block truncate font-semibold">{option.label}</span>
                  <span className="block truncate text-[11px] text-[#7A857E]">{option.routeId}</span>
                </span>
                {isSelected ? <Check aria-hidden="true" className="h-4 w-4 shrink-0" strokeWidth={1.9} /> : null}
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-[64px] rounded-[8px] border border-[#E0E7E0] bg-[#FBFCFA] px-3 py-2">
      <div className="text-[16px] font-semibold text-[#17251F]">{value}</div>
      <div className="text-[11px] text-[#7A857E]">{label}</div>
    </div>
  );
}

function EvidenceMeta({
  className = "",
  label,
  value
}: {
  className?: string;
  label: string;
  value: string;
}) {
  return (
    <div className={["min-w-0 rounded-[8px] border border-[#E0E7E0] bg-[#FBFCFA] px-3 py-2", className].join(" ")}>
      <div className="text-[11px] uppercase tracking-[0.04em] text-[#7A857E]">{label}</div>
      <div className="truncate text-[13px] font-semibold text-[#17251F]">{value}</div>
    </div>
  );
}

function rankingLabel(ranking: ResearchRanking) {
  return RANKINGS.find((option) => option.code === ranking)?.label ?? ranking;
}
