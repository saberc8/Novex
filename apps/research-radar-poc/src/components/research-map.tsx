"use client";

import { memo, useMemo, useState, type CSSProperties } from "react";
import {
  Background,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  type Edge,
  type Node,
  type NodeProps
} from "@xyflow/react";
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

type GraphLane = "center" | "questions" | "evidence" | "artifacts" | "experiments";

type PositionedNode = ResearchGraphNode & {
  x: number;
  y: number;
  category: "topic" | "hotspot" | "other";
  lane: GraphLane;
};

type ResearchFlowNodeData = {
  node: PositionedNode;
  copy: ResearchMapCopy;
  selected: boolean;
  accent: string;
  onSelect: (nodeId: string) => void;
};

type ResearchFlowNode = Node<ResearchFlowNodeData, "researchNode">;

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

const LANE_ORDER: Record<GraphLane, number> = {
  center: 0,
  questions: 1,
  evidence: 2,
  artifacts: 3,
  experiments: 4
};

const CATEGORY_ORDER: Record<PositionedNode["category"], number> = {
  topic: 0,
  hotspot: 1,
  other: 2
};

const FLOW_WIDTH = 1180;
const FLOW_HEIGHT = 820;

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

const nodeTypes = {
  researchNode: memo(ResearchFlowNodeView)
};

const invisibleHandleStyle: CSSProperties = {
  width: 8,
  height: 8,
  border: "none",
  background: "transparent",
  opacity: 0
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

  const flowNodes = useMemo<ResearchFlowNode[]>(
    () =>
      visibleNodes.map((node) => ({
        id: node.id,
        type: "researchNode",
        position: { x: node.x, y: node.y },
        data: {
          node,
          copy,
          selected: node.id === selectedNodeId,
          accent: accentForNode(node),
          onSelect: onNodeSelect
        },
        draggable: false,
        focusable: true,
        sourcePosition: sourcePositionForLane(node.lane),
        targetPosition: targetPositionForLane(node.lane),
        zIndex: node.id === selectedNodeId ? 10 : node.category === "topic" ? 8 : 1
      })),
    [copy, onNodeSelect, selectedNodeId, visibleNodes]
  );

  const flowEdges = useMemo<Edge[]>(
    () =>
      visibleEdges.map((edge) => ({
        id: edge.id,
        source: edge.from,
        target: edge.to,
        type: "smoothstep",
        label: copy.relationLabels[edge.relation],
        style: { stroke: "#B8CFC4", strokeWidth: 1.7 },
        labelStyle: {
          fill: "#5F6B64",
          fontSize: 10,
          fontWeight: 650
        },
        labelBgStyle: { fill: "#F9FBF8", fillOpacity: 0.94 },
        labelBgPadding: [6, 3],
        labelBgBorderRadius: 8,
        focusable: false,
        selectable: false
      })),
    [copy.relationLabels, visibleEdges]
  );

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
            <div className="relative h-[560px] min-h-[460px] overflow-hidden rounded-[8px] border border-[#E4EBE4] bg-[#F9FBF8] md:h-[640px]">
              <LaneFrame />
              <ReactFlowProvider
                initialNodes={flowNodes}
                initialEdges={flowEdges}
                fitView
                initialWidth={FLOW_WIDTH}
                initialHeight={640}
                initialFitViewOptions={{ padding: 0.18 }}
                nodeOrigin={[0.5, 0.5]}
              >
                <ReactFlow<ResearchFlowNode, Edge>
                  aria-label={copy.graphLabel}
                  className="research-flow"
                  nodes={flowNodes}
                  edges={flowEdges}
                  nodeTypes={nodeTypes}
                  fitView
                  fitViewOptions={{ padding: 0.18, duration: 260 }}
                  minZoom={0.35}
                  maxZoom={1.8}
                  nodeOrigin={[0.5, 0.5]}
                  nodesDraggable={false}
                  nodesConnectable={false}
                  edgesFocusable={false}
                  edgesReconnectable={false}
                  elementsSelectable={false}
                  onNodeClick={(_event, node) => onNodeSelect(node.id)}
                  panOnDrag
                  panOnScroll
                  zoomOnScroll
                  zoomOnPinch
                  proOptions={{ hideAttribution: true }}
                >
                  <Background color="#DCE7DF" gap={28} size={1} />
                  <MiniMap
                    pannable
                    zoomable
                    ariaLabel={`${copy.graphLabel} overview`}
                    nodeBorderRadius={8}
                    nodeColor={(node) => miniMapColorForNode(node as ResearchFlowNode)}
                    maskColor="rgba(238, 244, 240, 0.66)"
                    bgColor="#FBFCFA"
                  />
                  <Controls position="bottom-left" showInteractive={false} fitViewOptions={{ padding: 0.18 }} />
                </ReactFlow>
              </ReactFlowProvider>
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

function ResearchFlowNodeView({ data, sourcePosition, targetPosition }: NodeProps<ResearchFlowNode>) {
  const { node, copy, selected, accent, onSelect } = data;
  const Icon = KIND_ICON[node.kind];
  const nodeKind = copy.nodeKinds[node.kind];
  const title = `${node.title} - ${nodeKind} - ${node.summary}`;
  const widthClass = node.category === "topic" ? "w-[220px]" : "w-[204px]";

  return (
    <div className="relative">
      <Handle
        type="target"
        position={targetPosition ?? Position.Left}
        isConnectable={false}
        style={invisibleHandleStyle}
      />
      <button
        type="button"
        aria-pressed={selected}
        aria-label={node.title}
        title={title}
        className={[
          "nodrag nopan flex min-h-[58px] items-center gap-2 rounded-[8px] border bg-white px-3 py-2 text-left shadow-[0_12px_26px_rgba(34,45,38,0.10)] transition",
          widthClass,
          selected
            ? "border-[#0E6B5F] ring-2 ring-[#BFE5DC]"
            : "border-[#DCE5DD] hover:border-[#AFC4B8]"
        ].join(" ")}
        onClick={(event) => {
          event.stopPropagation();
          onSelect(node.id);
        }}
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
          <span className="mt-0.5 block truncate text-[10px] font-medium text-[#6B776F]">
            {nodeKind}
          </span>
        </span>
      </button>
      <Handle
        type="source"
        position={sourcePosition ?? Position.Right}
        isConnectable={false}
        style={invisibleHandleStyle}
      />
    </div>
  );
}

function LaneFrame() {
  return (
    <div aria-hidden="true" className="pointer-events-none absolute inset-0 z-0">
      {(Object.keys(LANE_ORDER) as GraphLane[]).map((lane) => {
        const frame = laneFrameForLane(lane);
        return (
          <div
            key={lane}
            className="absolute rounded-[8px] border border-dashed border-[#E2EAE4] bg-white/20"
            style={frame}
          />
        );
      })}
    </div>
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
    const leftLane = laneForKind(left.kind);
    const rightLane = laneForKind(right.kind);
    if (LANE_ORDER[leftLane] !== LANE_ORDER[rightLane]) {
      return LANE_ORDER[leftLane] - LANE_ORDER[rightLane];
    }

    const leftCategory = categoryForKind(left.kind);
    const rightCategory = categoryForKind(right.kind);
    if (CATEGORY_ORDER[leftCategory] !== CATEGORY_ORDER[rightCategory]) {
      return CATEGORY_ORDER[leftCategory] - CATEGORY_ORDER[rightCategory];
    }

    return right.importance - left.importance || left.title.localeCompare(right.title) || left.id.localeCompare(right.id);
  });

  const positioned: PositionedNode[] = [];
  const topicNodes = sorted.filter((node) => node.kind === "topic");
  const laneNodes = new Map<GraphLane, ResearchGraphNode[]>();

  for (const lane of Object.keys(LANE_ORDER) as GraphLane[]) {
    laneNodes.set(lane, []);
  }

  for (const node of sorted) {
    if (node.kind !== "topic") {
      laneNodes.get(laneForKind(node.kind))?.push(node);
    }
  }

  topicNodes.forEach((node, index) => {
    positioned.push({
      ...node,
      category: "topic",
      lane: "center",
      x: FLOW_WIDTH / 2,
      y: FLOW_HEIGHT / 2 + index * 92 - ((topicNodes.length - 1) * 92) / 2
    });
  });

  placeLane(laneNodes.get("questions") ?? [], "questions", positioned);
  placeLane(laneNodes.get("evidence") ?? [], "evidence", positioned);
  placeLane(laneNodes.get("artifacts") ?? [], "artifacts", positioned);
  placeLane(laneNodes.get("experiments") ?? [], "experiments", positioned);

  return positioned.sort((left, right) => left.id.localeCompare(right.id));
}

function placeLane(
  nodes: ResearchGraphNode[],
  lane: GraphLane,
  positioned: PositionedNode[]
) {
  if (nodes.length === 0) {
    return;
  }

  const bounds = boundsForLane(lane);
  const columns = lane === "evidence" || lane === "artifacts" ? Math.min(2, nodes.length) : Math.min(4, nodes.length);
  const rows = Math.ceil(nodes.length / columns);
  const xGap = columns > 1 ? bounds.width / (columns - 1) : 0;
  const yGap = rows > 1 ? bounds.height / (rows - 1) : 0;

  nodes.forEach((node, index) => {
    const column = index % columns;
    const row = Math.floor(index / columns);
    const columnOffset = rows > 1 && row % 2 === 1 ? Math.min(46, xGap / 3) : 0;

    positioned.push({
      ...node,
      category: categoryForKind(node.kind),
      lane,
      x: clamp(bounds.x + column * xGap + columnOffset, bounds.x, bounds.x + bounds.width),
      y: clamp(bounds.y + row * yGap, bounds.y, bounds.y + bounds.height)
    });
  });
}

function laneForKind(kind: ResearchGraphNodeKind): GraphLane {
  if (kind === "topic") {
    return "center";
  }

  if (kind === "hotspot" || kind === "open_question") {
    return "questions";
  }

  if (kind === "paper" || kind === "author" || kind === "institution") {
    return "evidence";
  }

  if (kind === "experiment" || kind === "benchmark") {
    return "experiments";
  }

  return "artifacts";
}

function boundsForLane(lane: GraphLane) {
  switch (lane) {
    case "questions":
      return { x: 270, y: 92, width: 640, height: 122 };
    case "evidence":
      return { x: 122, y: 268, width: 300, height: 322 };
    case "artifacts":
      return { x: 760, y: 268, width: 300, height: 322 };
    case "experiments":
      return { x: 270, y: 646, width: 640, height: 96 };
    case "center":
    default:
      return { x: 470, y: 322, width: 240, height: 170 };
  }
}

function laneFrameForLane(lane: GraphLane): CSSProperties {
  const bounds = boundsForLane(lane);
  return {
    left: `${(bounds.x - 118) / FLOW_WIDTH * 100}%`,
    top: `${(bounds.y - 54) / FLOW_HEIGHT * 100}%`,
    width: `${(bounds.width + 236) / FLOW_WIDTH * 100}%`,
    height: `${(bounds.height + 108) / FLOW_HEIGHT * 100}%`
  };
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

function sourcePositionForLane(lane: GraphLane): Position {
  if (lane === "evidence") {
    return Position.Right;
  }

  if (lane === "artifacts") {
    return Position.Left;
  }

  if (lane === "questions") {
    return Position.Bottom;
  }

  if (lane === "experiments") {
    return Position.Top;
  }

  return Position.Right;
}

function targetPositionForLane(lane: GraphLane): Position {
  if (lane === "evidence") {
    return Position.Right;
  }

  if (lane === "artifacts") {
    return Position.Left;
  }

  if (lane === "questions") {
    return Position.Bottom;
  }

  if (lane === "experiments") {
    return Position.Top;
  }

  return Position.Left;
}

function accentForNode(node: PositionedNode): string {
  if (node.category === "topic") {
    return "#0E6B5F";
  }

  if (node.category === "hotspot") {
    return "#1F7C6D";
  }

  if (node.kind === "paper" || node.kind === "author" || node.kind === "institution") {
    return "#496E8D";
  }

  if (node.kind === "experiment" || node.kind === "benchmark") {
    return "#9A650F";
  }

  return "#536A86";
}

function miniMapColorForNode(node: ResearchFlowNode): string {
  return accentForNode(node.data.node);
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
