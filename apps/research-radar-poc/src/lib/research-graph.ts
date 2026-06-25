import type {
  ParsedResearchReport,
  ResearchGraph,
  ResearchGraphEdge,
  ResearchGraphNode,
  ResearchGraphNodeKind,
  ResearchGraphRelation,
  ResearchSourceItem,
  ResearchSourceScanResp
} from "@/types/research";

export type BuildResearchGraphInput = {
  topic: string;
  sourceScan?: ResearchSourceScanResp | null;
  parsedReport: ParsedResearchReport;
  finalOutput: string;
};

export type ResearchGraphNodeConnection = {
  node: ResearchGraphNode;
  relation: ResearchGraphRelation;
  direction: "incoming" | "outgoing";
  evidenceItemIds: string[];
};

export type ResearchGraphNodeDetails = {
  node: ResearchGraphNode;
  connectedNodes: ResearchGraphNodeConnection[];
  sourceItemIds: string[];
  sourceItems: ResearchSourceItem[];
  caveats: string[];
  suggestedNextAction: string;
};

const GRAPH_BLOCK_PATTERN = /```research-graph-json\s*([\s\S]*?)```/i;
const MAX_HOTSPOTS = 6;
const MAX_REPORT_DERIVED_NODES = 4;

const NODE_KINDS = new Set<ResearchGraphNodeKind>([
  "topic",
  "hotspot",
  "paper",
  "project",
  "model",
  "dataset",
  "benchmark",
  "author",
  "institution",
  "open_question",
  "experiment"
]);

const EDGE_RELATIONS = new Set<ResearchGraphRelation>([
  "supports",
  "implements",
  "evaluates",
  "extends",
  "reveals_gap",
  "leads_to",
  "mentions"
]);

export function parseResearchGraphBlock(markdown: string): ResearchGraph | null {
  const match = markdown.match(GRAPH_BLOCK_PATTERN);
  if (!match?.[1]) {
    return null;
  }

  try {
    return normalizeGraph(JSON.parse(match[1]));
  } catch {
    return null;
  }
}

export function buildResearchGraph(input: BuildResearchGraphInput): ResearchGraph {
  const sourceDerivedGraph = buildSourceDerivedGraph(input);
  const parsed = parseResearchGraphBlock(input.finalOutput);
  if (parsed) {
    const repairedGraph = repairParsedGraph(parsed, sourceDerivedGraph);
    if (!repairedGraph) {
      return sourceDerivedGraph;
    }

    const graph = {
      ...repairedGraph,
      caveats: uniqueStrings([...repairedGraph.caveats, ...sourceDerivedGraph.caveats])
    };

    if (hasUsableGraphNodes(graph) || !hasUsableGraphNodes(sourceDerivedGraph)) {
      return graph;
    }
  }

  return sourceDerivedGraph;
}

export function nodeDetailsFor(
  graph: ResearchGraph,
  nodeId: string,
  sourceScan?: ResearchSourceScanResp | null
): ResearchGraphNodeDetails | null {
  const node = graph.nodes.find((candidate) => candidate.id === nodeId);
  if (!node) {
    return null;
  }

  const connectedNodes = graph.edges
    .filter((edge) => edge.from === nodeId || edge.to === nodeId)
    .flatMap((edge) => {
      const connectedNodeId = edge.from === nodeId ? edge.to : edge.from;
      const connectedNode = graph.nodes.find((candidate) => candidate.id === connectedNodeId);
      if (!connectedNode) {
        return [];
      }

      const direction: ResearchGraphNodeConnection["direction"] =
        edge.from === nodeId ? "outgoing" : "incoming";

      return [{
        node: connectedNode,
        relation: edge.relation,
        direction,
        evidenceItemIds: edge.evidenceItemIds
      }];
    });

  const sourceItemsById = new Map(allSourceItems(sourceScan).map((item) => [item.id, item]));
  const sourceItemIds = uniqueStrings([
    ...node.sourceItemIds,
    ...connectedNodes.flatMap((connection) => connection.node.sourceItemIds),
    ...connectedNodes.flatMap((connection) => connection.evidenceItemIds)
  ]);

  return {
    node,
    connectedNodes,
    sourceItemIds,
    sourceItems: sourceItemIds
      .map((sourceItemId) => sourceItemsById.get(sourceItemId))
      .filter((sourceItem): sourceItem is ResearchSourceItem => Boolean(sourceItem)),
    caveats: graph.caveats,
    suggestedNextAction: suggestedNextActionFor(node)
  };
}

function buildSourceDerivedGraph(input: BuildResearchGraphInput): ResearchGraph {
  const topicNode = topicGraphNode(input.topic);
  const sourceItems = allSourceItems(input.sourceScan);
  const caveats = sourceGraphCaveats(input.sourceScan);
  const hotspots = buildHotspotNodes(input.topic, sourceItems);
  const evidenceNodes = sourceItems.map(sourceItemToNode);
  const reportNodes = reportDerivedNodes(input.parsedReport);

  const nodes = [topicNode, ...hotspots, ...evidenceNodes, ...reportNodes];
  const edges = [
    ...hotspots.map((hotspot) => edgeFor(topicNode.id, hotspot.id, "mentions", [])),
    ...evidenceNodes.map((node) => {
      const hotspot = strongestHotspotFor(node, hotspots);
      return edgeFor(hotspot?.id ?? topicNode.id, node.id, relationForNode(node), node.sourceItemIds);
    }),
    ...reportNodes.map((node) =>
      edgeFor(topicNode.id, node.id, node.kind === "experiment" ? "leads_to" : "reveals_gap", node.sourceItemIds)
    )
  ];

  return {
    topic: topicNode.title,
    nodes,
    edges,
    caveats
  };
}

function sourceGraphCaveats(sourceScan?: ResearchSourceScanResp | null) {
  return uniqueStrings([
    ...(sourceScan?.warnings ?? []),
    ...(sourceScan?.sources.flatMap((source) => (source.warning ? [source.warning] : [])) ?? [])
  ]);
}

function allSourceItems(sourceScan?: ResearchSourceScanResp | null) {
  const items = [...(sourceScan?.items ?? []), ...(sourceScan?.sources.flatMap((source) => source.items) ?? [])];
  const itemsById = new Map<string, ResearchSourceItem>();

  items.forEach((item) => {
    if (!itemsById.has(item.id)) {
      itemsById.set(item.id, item);
    }
  });

  return [...itemsById.values()];
}

function hasUsableGraphNodes(graph: ResearchGraph) {
  return graph.nodes.some((node) => node.kind !== "topic");
}

function repairParsedGraph(parsed: ResearchGraph, sourceDerivedGraph: ResearchGraph): ResearchGraph | null {
  const sourceTopicNode = sourceDerivedGraph.nodes.find((node) => node.kind === "topic");
  const nodesById = new Map(parsed.nodes.map((node) => [node.id, node]));

  if (!parsed.nodes.some((node) => node.kind === "topic")) {
    if (!sourceTopicNode) {
      return null;
    }
    nodesById.set(sourceTopicNode.id, sourceTopicNode);
  }

  const topicNode = [...nodesById.values()].find((node) => node.kind === "topic");
  if (!topicNode) {
    return null;
  }

  const repairedEdges: ResearchGraphEdge[] = [];
  for (const edge of parsed.edges) {
    const from = repairEdgeEndpoint(edge.from, nodesById, topicNode.id);
    const to = repairEdgeEndpoint(edge.to, nodesById, topicNode.id);
    if (!from || !to) {
      return null;
    }

    repairedEdges.push({
      ...edge,
      id: `${from}->${to}:${edge.relation}`,
      from,
      to,
      evidenceItemIds: uniqueStrings(edge.evidenceItemIds)
    });
  }

  if (!repairedEdges.some((edge) => edge.from === topicNode.id || edge.to === topicNode.id)) {
    const rootNodes = [...nodesById.values()].filter((node) => {
      if (node.id === topicNode.id) {
        return false;
      }

      return !repairedEdges.some((edge) => edge.to === node.id);
    });
    const targets = rootNodes.length > 0
      ? rootNodes
      : [...nodesById.values()].filter((node) => node.id !== topicNode.id);

    targets.forEach((node) => {
      repairedEdges.push(edgeFor(topicNode.id, node.id, relationFromTopic(node), node.sourceItemIds));
    });
  }

  return {
    topic: parsed.topic,
    nodes: [...nodesById.values()],
    edges: uniqueEdges(repairedEdges),
    caveats: parsed.caveats
  };
}

function normalizeGraph(value: unknown): ResearchGraph | null {
  if (!value || typeof value !== "object") {
    return null;
  }

  const graph = value as Partial<ResearchGraph>;
  if (!isNonEmptyString(graph.topic) || !Array.isArray(graph.nodes) || !Array.isArray(graph.edges)) {
    return null;
  }

  const nodes = graph.nodes.map(normalizeNode);
  if (nodes.some((node) => node === null)) {
    return null;
  }

  const edges = graph.edges.map(normalizeEdge);
  if (edges.some((edge) => edge === null)) {
    return null;
  }

  return {
    topic: graph.topic.trim(),
    nodes: nodes as ResearchGraphNode[],
    edges: edges as ResearchGraphEdge[],
    caveats: Array.isArray(graph.caveats) ? uniqueStrings(graph.caveats.map((value) => String(value))) : []
  };
}

function normalizeNode(value: unknown): ResearchGraphNode | null {
  if (!value || typeof value !== "object") {
    return null;
  }

  const node = value as Partial<ResearchGraphNode>;
  if (!isNonEmptyString(node.id) || !isNonEmptyString(node.title) || !isResearchGraphNodeKind(node.kind)) {
    return null;
  }

  return {
    id: node.id.trim(),
    kind: node.kind,
    title: node.title.trim(),
    summary: typeof node.summary === "string" ? node.summary : "",
    importance: typeof node.importance === "number" && Number.isFinite(node.importance) ? node.importance : 0.5,
    recency: typeof node.recency === "string" ? node.recency : null,
    sourceItemIds: Array.isArray(node.sourceItemIds) ? node.sourceItemIds.map((value) => String(value)) : [],
    tags: Array.isArray(node.tags) ? node.tags.map((value) => String(value)) : []
  };
}

function normalizeEdge(value: unknown): ResearchGraphEdge | null {
  if (!value || typeof value !== "object") {
    return null;
  }

  const edge = value as Partial<ResearchGraphEdge>;
  if (!isNonEmptyString(edge.id) || !isNonEmptyString(edge.from) || !isNonEmptyString(edge.to) || !isResearchGraphRelation(edge.relation)) {
    return null;
  }

  return {
    id: edge.id.trim(),
    from: edge.from.trim(),
    to: edge.to.trim(),
    relation: edge.relation,
    evidenceItemIds: Array.isArray(edge.evidenceItemIds) ? edge.evidenceItemIds.map((value) => String(value)) : []
  };
}

function topicGraphNode(topic: string): ResearchGraphNode {
  const title = topic.trim() || "Research Topic";
  return {
    id: `topic:${slug(title)}`,
    kind: "topic",
    title,
    summary: "Central research point for this radar scan.",
    importance: 1,
    sourceItemIds: [],
    tags: []
  };
}

function buildHotspotNodes(topic: string, items: ResearchSourceItem[]): ResearchGraphNode[] {
  const topicSlug = slug(topic);
  const counts = new Map<string, number>();

  items.forEach((item) => {
    const terms = [...item.tags, ...titleTerms(item.title)].filter((term) => term !== topicSlug);
    terms.forEach((term) => counts.set(term, (counts.get(term) ?? 0) + 1));
  });

  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, MAX_HOTSPOTS)
    .map(([term, count]) => ({
      id: `hotspot:${slug(term)}`,
      kind: "hotspot",
      title: term,
      summary: `Recurring signal across ${count} source item${count === 1 ? "" : "s"}.`,
      importance: Math.min(1, 0.35 + count * 0.15),
      sourceItemIds: items.filter((item) => itemMatchesTerm(item, term)).map((item) => item.id),
      tags: [term]
    }));
}

function sourceItemToNode(item: ResearchSourceItem): ResearchGraphNode {
  return {
    id: `source:${item.id}`,
    kind: nodeKindForSourceItem(item),
    title: item.title,
    summary: item.summary ?? item.organization ?? item.authors.slice(0, 3).join(", "),
    importance: importanceFromMetrics(item),
    recency: item.publishedAt ?? item.updatedAt ?? null,
    sourceItemIds: [item.id],
    tags: item.tags
  };
}

function reportDerivedNodes(parsedReport: ParsedResearchReport): ResearchGraphNode[] {
  const openings = reportBullets(parsedReport, "research-openings", "open_question");
  const experiments = reportBullets(parsedReport, "experiment-plans", "experiment");
  return [...openings, ...experiments].slice(0, MAX_REPORT_DERIVED_NODES);
}

function reportBullets(
  parsedReport: ParsedResearchReport,
  sectionId: string,
  kind: Extract<ResearchGraphNodeKind, "open_question" | "experiment">
): ResearchGraphNode[] {
  const section = parsedReport.sections.find((item) => item.id === sectionId);
  if (!section?.content) {
    return [];
  }

  return section.content
    .split("\n")
    .map((line) => line.replace(/^[-*]\s*/, "").trim())
    .filter(Boolean)
    .slice(0, 2)
    .map((title) => ({
      id: `${kind}:${slug(title)}`,
      kind,
      title,
      summary:
        kind === "experiment"
          ? "Candidate experiment plan from the analysis report."
          : "Open research question from the analysis report.",
      importance: 0.7,
      sourceItemIds: [],
      tags: []
    }));
}

function strongestHotspotFor(node: ResearchGraphNode, hotspots: ResearchGraphNode[]) {
  return hotspots.find((hotspot) => node.tags.includes(hotspot.title) || node.title.toLowerCase().includes(hotspot.title));
}

function relationForNode(node: ResearchGraphNode): ResearchGraphRelation {
  if (node.kind === "project" || node.kind === "model") {
    return "implements";
  }
  if (node.kind === "dataset" || node.kind === "benchmark") {
    return "evaluates";
  }
  return "supports";
}

function relationFromTopic(node: ResearchGraphNode): ResearchGraphRelation {
  if (node.kind === "hotspot" || node.kind === "topic" || node.kind === "author" || node.kind === "institution") {
    return "mentions";
  }
  if (node.kind === "open_question") {
    return "reveals_gap";
  }
  if (node.kind === "experiment") {
    return "leads_to";
  }

  return relationForNode(node);
}

function edgeFor(from: string, to: string, relation: ResearchGraphRelation, evidenceItemIds: string[]): ResearchGraphEdge {
  return {
    id: `${from}->${to}:${relation}`,
    from,
    to,
    relation,
    evidenceItemIds
  };
}

function nodeKindForSourceItem(item: ResearchSourceItem): ResearchGraphNodeKind {
  if (item.kind === "project") {
    return "project";
  }
  if (item.kind === "model") {
    return "model";
  }
  if (item.kind === "dataset") {
    return "dataset";
  }
  if (item.kind === "benchmark") {
    return "benchmark";
  }
  return "paper";
}

function repairEdgeEndpoint(endpoint: string, nodesById: Map<string, ResearchGraphNode>, topicNodeId: string) {
  if (nodesById.has(endpoint)) {
    return endpoint;
  }

  if (endpoint.startsWith("topic:")) {
    return topicNodeId;
  }

  return null;
}

function importanceFromMetrics(item: ResearchSourceItem) {
  const total = item.metrics.reduce((sum, metric) => sum + Math.max(0, metric.value), 0);
  return Math.min(1, total > 0 ? 0.45 + Math.log10(total + 1) / 8 : 0.45);
}

function itemMatchesTerm(item: ResearchSourceItem, term: string) {
  return item.tags.includes(term) || item.title.toLowerCase().includes(term);
}

function titleTerms(title: string) {
  return title
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((term) => term.length >= 5)
    .slice(0, 4);
}

function slug(value: string) {
  return value
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function isResearchGraphNodeKind(value: unknown): value is ResearchGraphNodeKind {
  return typeof value === "string" && NODE_KINDS.has(value as ResearchGraphNodeKind);
}

function isResearchGraphRelation(value: unknown): value is ResearchGraphRelation {
  return typeof value === "string" && EDGE_RELATIONS.has(value as ResearchGraphRelation);
}

function uniqueStrings(values: string[]) {
  return values.map((value) => value.trim()).filter((value, index, all) => value.length > 0 && all.indexOf(value) === index);
}

function uniqueEdges(edges: ResearchGraphEdge[]) {
  const edgesById = new Map<string, ResearchGraphEdge>();
  edges.forEach((edge) => {
    if (!edgesById.has(edge.id)) {
      edgesById.set(edge.id, edge);
    }
  });

  return [...edgesById.values()];
}

function suggestedNextActionFor(node: ResearchGraphNode) {
  switch (node.kind) {
    case "topic":
      return "Open the strongest hotspot first, then trace the evidence nodes attached to it.";
    case "hotspot":
      return "Open the linked evidence and compare the strongest papers, projects, or benchmarks behind this theme.";
    case "paper":
      return "Read the abstract and method summary first, then trace any connected implementations or benchmarks.";
    case "project":
    case "model":
      return "Open the source and inspect the implementation surface before adding it to the reading route.";
    case "dataset":
    case "benchmark":
      return "Check the task setup, metrics, and coverage before using this as an evaluation anchor.";
    case "open_question":
      return "Collect the conflicting evidence around this gap and define one validation criterion.";
    case "experiment":
      return "Turn this into a scoped run plan with one dataset, one metric, and one comparison target.";
    case "author":
    case "institution":
      return "Trace the connected work to see where this person or lab is shaping the landscape.";
    default:
      return "Follow the connected evidence to refine the next reading or evaluation step.";
  }
}
