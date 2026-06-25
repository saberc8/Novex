"use client";

import { useMemo, useState } from "react";
import {
  Beaker,
  BookOpen,
  Boxes,
  Database,
  GitBranch,
  HelpCircle,
  Network,
  Orbit,
  Package,
  Users
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { ResearchMapCopy } from "@/lib/i18n";
import type {
  ResearchGraph,
  ResearchGraphNode,
  ResearchGraphNodeKind
} from "@/types/research";

export type ResearchMapProps = {
  graph: ResearchGraph;
  selectedNodeId: string | null;
  onNodeSelect: (nodeId: string) => void;
  copy?: ResearchMapCopy;
};

type MapLayer = keyof ResearchMapCopy["layers"];

type PositionedNode = ResearchGraphNode & {
  x: number;
  y: number;
  radius: number;
  category: "topic" | "hotspot" | "other";
};

const LAYERS: Array<{ layer: MapLayer; kinds: ResearchGraphNodeKind[] }> = [
  { layer: "papers", kinds: ["paper"] },
  { layer: "people", kinds: ["author", "institution"] },
  { layer: "projects", kinds: ["project"] },
  { layer: "models", kinds: ["model"] },
  { layer: "datasets", kinds: ["dataset"] },
  { layer: "benchmarks", kinds: ["benchmark"] },
  { layer: "questions", kinds: ["open_question"] },
  { layer: "experiments", kinds: ["experiment"] }
];

const KIND_ICON: Record<ResearchGraphNodeKind, LucideIcon> = {
  topic: Orbit,
  hotspot: Network,
  paper: BookOpen,
  project: GitBranch,
  model: Package,
  dataset: Database,
  benchmark: Boxes,
  author: Users,
  institution: Users,
  open_question: HelpCircle,
  experiment: Beaker
};

const CATEGORY_ORDER: Record<PositionedNode["category"], number> = {
  topic: 0,
  hotspot: 1,
  other: 2
};

const DEFAULT_MAP_COPY: ResearchMapCopy = {
  title: "Research Map",
  description: "Explore how topics, evidence, gaps, and experiments connect.",
  graphLabel: "Research graph",
  nodeCount: (count) => `${count} nodes`,
  noUsableNodes: "No usable graph nodes",
  noUsableNodesDescription: "Source warnings and caveats are listed below when coverage is limited.",
  caveats: "Caveats",
  layers: {
    papers: "Papers",
    people: "People",
    projects: "Projects",
    models: "Models",
    datasets: "Datasets",
    benchmarks: "Benchmarks",
    questions: "Questions",
    experiments: "Experiments"
  },
  nodeKinds: {
    topic: "topic",
    hotspot: "hotspot",
    paper: "paper",
    project: "project",
    model: "model",
    dataset: "dataset",
    benchmark: "benchmark",
    author: "author",
    institution: "institution",
    open_question: "open question",
    experiment: "experiment"
  },
  relationLabels: {
    supports: "supports",
    implements: "implements",
    evaluates: "evaluates",
    extends: "extends",
    reveals_gap: "reveals gap",
    leads_to: "leads to",
    mentions: "mentions"
  }
};

export function ResearchMap({
  graph,
  selectedNodeId,
  onNodeSelect,
  copy = DEFAULT_MAP_COPY
}: ResearchMapProps) {
  const [enabledLayers, setEnabledLayers] = useState<Set<MapLayer>>(
    () => new Set(LAYERS.map((layer) => layer.layer))
  );

  const hasUsableNodes = graph.nodes.some((node) => node.kind !== "topic");
  const positionedNodes = useMemo(() => layoutGraph(graph.nodes), [graph.nodes]);
  const visibleNodes = positionedNodes.filter(
    (node) => node.category !== "other" || nodeVisibleForLayers(node, enabledLayers)
  );
  const visibleIds = new Set(visibleNodes.map((node) => node.id));
  const visibleEdges = graph.edges.filter((edge) => visibleIds.has(edge.from) && visibleIds.has(edge.to));

  return (
    <section className="rounded-[8px] border border-[#DCE5DD] bg-white p-5 text-[#17251F] shadow-[0_10px_24px_rgba(34,45,38,0.05)]">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <h3 className="flex items-center gap-2 text-[16px] font-semibold">
            <Network aria-hidden="true" className="h-4 w-4 text-[#0E6B5F]" strokeWidth={1.9} />
            {copy.title}
          </h3>
          <p className="mt-1 text-[12px] text-[#6B776F]">{copy.description}</p>
        </div>

        <div className="rounded-[7px] bg-[#EEF3ED] px-2 py-1 text-[12px] text-[#66736B]">
          {copy.nodeCount(graph.nodes.length)}
        </div>
      </div>

      {hasUsableNodes ? (
        <>
          <div className="mt-4 flex flex-wrap gap-2">
            {LAYERS.map((layer) => {
              const active = enabledLayers.has(layer.layer);
              return (
                <button
                  key={layer.layer}
                  type="button"
                  aria-pressed={active}
                  className={[
                    "inline-flex h-8 items-center rounded-[8px] border px-2.5 text-[12px] font-medium transition",
                    active
                      ? "border-[#0E6B5F] bg-[#E9F7F3] text-[#0B5D53]"
                      : "border-[#DDE5DD] bg-white text-[#66736B]"
                  ].join(" ")}
                  onClick={() => {
                    setEnabledLayers((current) => {
                      const next = new Set(current);
                      if (next.has(layer.layer)) {
                        next.delete(layer.layer);
                      } else {
                        next.add(layer.layer);
                      }
                      return next;
                    });
                  }}
                >
                  {copy.layers[layer.layer]}
                </button>
              );
            })}
          </div>

          <div className="mt-5">
            <div
              aria-label={copy.graphLabel}
              className="relative aspect-[16/10] min-h-[420px] overflow-hidden rounded-[8px] border border-[#E4EBE4] bg-[#F9FBF8]"
            >
              <svg
                aria-hidden="true"
                className="absolute inset-0 h-full w-full"
                viewBox="0 0 1000 625"
                preserveAspectRatio="none"
              >
                <defs>
                  <linearGradient id="research-map-edge" x1="0%" x2="100%" y1="0%" y2="0%">
                    <stop offset="0%" stopColor="#BFD4CB" />
                    <stop offset="100%" stopColor="#D9E3DA" />
                  </linearGradient>
                </defs>
                {visibleEdges.map((edge) => {
                  const from = positionedNodes.find((node) => node.id === edge.from);
                  const to = positionedNodes.find((node) => node.id === edge.to);
                  if (!from || !to) {
                    return null;
                  }

                  const midX = (from.x + to.x) / 2;
                  const midY = (from.y + to.y) / 2;
                  const angle = Math.atan2(to.y - from.y, to.x - from.x);

                  return (
                    <g key={edge.id}>
                      <line
                        x1={from.x}
                        y1={from.y}
                        x2={to.x}
                        y2={to.y}
                        stroke="url(#research-map-edge)"
                        strokeWidth="2"
                        strokeLinecap="round"
                      />
                      <circle
                        cx={midX}
                        cy={midY}
                        r="12"
                        fill="#F9FBF8"
                        opacity="0.92"
                        transform={`rotate(${(angle * 180) / Math.PI} ${midX} ${midY})`}
                      />
                    </g>
                  );
                })}
              </svg>

              {visibleEdges.map((edge) => {
                const from = positionedNodes.find((node) => node.id === edge.from);
                const to = positionedNodes.find((node) => node.id === edge.to);
                if (!from || !to) {
                  return null;
                }

                return (
                  <span
                    key={`${edge.id}:label`}
                    className="pointer-events-none absolute -translate-x-1/2 -translate-y-1/2 rounded-full border border-[#DCE5DD] bg-white px-2 py-[2px] text-[10px] font-medium uppercase tracking-[0.08em] text-[#627068] shadow-[0_4px_10px_rgba(34,45,38,0.06)]"
                    style={{
                      left: `${(from.x + to.x) / 2 / 10}%`,
                      top: `${(from.y + to.y) / 2 / 6.25}%`
                    }}
                  >
                    {copy.relationLabels[edge.relation]}
                  </span>
                );
              })}

              {visibleNodes.map((node) => {
                const Icon = KIND_ICON[node.kind];
                const selected = node.id === selectedNodeId;
                const accent = node.category === "topic"
                  ? "#0E6B5F"
                  : node.category === "hotspot"
                    ? "#1F7C6D"
                    : "#4E6A85";
                const nodeKind = copy.nodeKinds[node.kind];
                const title = `${node.title} - ${nodeKind} - ${node.summary}`;

                return (
                  <button
                    key={node.id}
                    type="button"
                    aria-pressed={selected}
                    aria-label={node.title}
                    title={title}
                    className={[
                      "absolute flex -translate-x-1/2 -translate-y-1/2 items-center gap-2 rounded-[8px] border px-3 py-2 text-left shadow-[0_8px_18px_rgba(34,45,38,0.08)] transition",
                      selected
                        ? "border-[#0E6B5F] bg-white ring-2 ring-[#BFE5DC]"
                        : "border-[#DCE5DD] bg-white hover:border-[#B6C7BA]"
                    ].join(" ")}
                    style={{
                      left: `${(node.x / 1000) * 100}%`,
                      top: `${(node.y / 625) * 100}%`,
                      minWidth: node.category === "topic" ? "186px" : "174px",
                      zIndex: selected ? 3 : 2
                    }}
                    onClick={() => onNodeSelect(node.id)}
                  >
                    <span
                      className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[7px] text-white"
                      style={{ backgroundColor: accent }}
                      aria-hidden="true"
                    >
                      <Icon className="h-4 w-4" strokeWidth={2} />
                    </span>
                    <span className="min-w-0">
                      <span className="block truncate text-[12px] font-semibold text-[#17251F]">
                        {node.title}
                      </span>
                      <span className="block truncate text-[10px] uppercase tracking-[0.08em] text-[#6B776F]">
                        {nodeKind}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        </>
      ) : (
        <div className="mt-5 rounded-[8px] border border-dashed border-[#D7E0D7] bg-[#FBFCFA] px-4 py-5">
          <p className="text-[14px] font-medium text-[#17251F]">{copy.noUsableNodes}</p>
          <p className="mt-1 text-[12px] leading-5 text-[#6B776F]">
            {copy.noUsableNodesDescription}
          </p>
        </div>
      )}

      {graph.caveats.length > 0 ? (
        <div className="mt-4 rounded-[8px] border border-[#E5ECE5] bg-[#FAFCF9] px-3 py-2 text-[12px] text-[#5F6B64]">
          <span className="font-medium text-[#49564F]">{copy.caveats}:</span> {graph.caveats.join("; ")}
        </div>
      ) : null}
    </section>
  );
}

function nodeVisibleForLayers(
  node: ResearchGraphNode,
  enabledLayers: Set<MapLayer>
): boolean {
  const layer = layerForKind(node.kind);
  return layer === null ? true : enabledLayers.has(layer);
}

function layerForKind(kind: ResearchGraphNodeKind): MapLayer | null {
  for (const layer of LAYERS) {
    if (layer.kinds.includes(kind)) {
      return layer.layer;
    }
  }
  return null;
}

function layoutGraph(nodes: ResearchGraphNode[]): PositionedNode[] {
  const sorted = [...nodes].sort((left, right) => {
    const leftCategory = categoryForKind(left.kind);
    const rightCategory = categoryForKind(right.kind);
    if (CATEGORY_ORDER[leftCategory] !== CATEGORY_ORDER[rightCategory]) {
      return CATEGORY_ORDER[leftCategory] - CATEGORY_ORDER[rightCategory];
    }

    return left.title.localeCompare(right.title) || left.id.localeCompare(right.id);
  });

  const width = 1000;
  const height = 625;
  const centerX = width / 2;
  const centerY = height / 2;
  const topicRadius = 58;
  const hotspotRadius = 48;
  const otherRadius = 42;
  const innerRing = 148;
  const outerRing = 248;

  const topicNodes = sorted.filter((node) => node.kind === "topic");
  const hotspotNodes = sorted.filter((node) => node.kind === "hotspot");
  const otherNodes = sorted.filter((node) => node.kind !== "topic" && node.kind !== "hotspot");

  const positioned: PositionedNode[] = [];

  topicNodes.forEach((node, index) => {
    positioned.push({
      ...node,
      category: "topic",
      radius: topicRadius,
      x: centerX,
      y: centerY + index * 92 - ((topicNodes.length - 1) * 92) / 2
    });
  });

  placeRing(hotspotNodes, innerRing, hotspotRadius, "hotspot", positioned, centerX, centerY);
  placeRing(otherNodes, outerRing, otherRadius, "other", positioned, centerX, centerY);

  return positioned.sort((left, right) => left.id.localeCompare(right.id));
}

function placeRing(
  nodes: ResearchGraphNode[],
  radius: number,
  nodeRadius: number,
  category: PositionedNode["category"],
  positioned: PositionedNode[],
  centerX: number,
  centerY: number
) {
  if (nodes.length === 0) {
    return;
  }

  nodes.forEach((node, index) => {
    const angle = (Math.PI * 2 * index) / nodes.length - Math.PI / 2;
    positioned.push({
      ...node,
      category,
      radius: nodeRadius,
      x: clamp(centerX + Math.cos(angle) * radius, 76, 924),
      y: clamp(centerY + Math.sin(angle) * radius, 72, 553)
    });
  });
}

function categoryForKind(kind: ResearchGraphNodeKind): PositionedNode["category"] {
  if (kind === "topic") {
    return "topic";
  }

  if (kind === "hotspot") {
    return "hotspot";
  }

  return "other";
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
