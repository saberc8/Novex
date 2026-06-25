# Research Radar Graph Analysis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the Research Radar POC from a source-result list into a graph-first research analysis workflow.

**Architecture:** Keep source scanning and Agent runs as they are, but add a frontend graph layer that can parse model-provided graph JSON and fall back to deterministic source-derived graph construction. The page will render the graph as the primary artifact, move raw sources into a collapsible evidence drawer, and use the right rail as a node inspector.

**Tech Stack:** Next.js 16, React 19, TypeScript, Tailwind CSS, lucide-react, Vitest, Testing Library. No new graph dependency for the first pass; use deterministic SVG/HTML layout for stable tests.

## Global Constraints

- Main display must be graph-first, not a raw source list.
- Raw source results remain available in a collapsible Evidence Drawer.
- The graph must degrade gracefully when the Agent run fails or graph JSON is malformed.
- Agent input must remain within the backend 4000 character limit.
- Implementation must follow existing Research Radar POC patterns and avoid unrelated backend changes.
- Preserve current source scan behavior for arXiv, GitHub, HuggingFace, PapersWithCode-compatible endpoints, and leaderboard endpoints.
- Keep UI dense, work-focused, and readable; no marketing hero or decorative layout.
- Existing user change `apps/codex-app-poc/app/layout.tsx` must not be modified, staged, or reverted.

---

## File Structure

- Modify `apps/research-radar-poc/src/types/research.ts`
  - Add graph node, edge, graph, and layer types.
- Create `apps/research-radar-poc/src/lib/research-graph.ts`
  - Parse `research-graph-json` fenced blocks.
  - Build deterministic fallback graph from `ResearchSourceScanResp`, source items, warnings, and parsed report sections.
  - Produce node details for the inspector.
- Create `apps/research-radar-poc/src/lib/research-graph.test.ts`
  - Unit tests for graph parsing, malformed JSON fallback, source-derived graph generation, caveats, and node detail lookup.
- Modify `apps/research-radar-poc/src/api/research.ts`
  - Ask the Agent for a compact graph JSON block before the existing markdown report.
  - Keep the 4000 character input budget.
- Modify `apps/research-radar-poc/src/api/research.test.ts`
  - Assert graph JSON instructions are present and the input cap still holds.
- Create `apps/research-radar-poc/src/components/research-map.tsx`
  - SVG/HTML graph visualization with layer toggles, selected-node state, hover labels, and accessible buttons.
- Create `apps/research-radar-poc/src/components/research-map.test.tsx`
  - Component tests for rendering nodes, layer toggles, and node selection.
- Modify `apps/research-radar-poc/src/app-client.tsx`
  - Build graph from the active scan.
  - Render `Research Map` before evidence.
  - Move `Source Results` into a collapsible drawer.
  - Replace the right rail empty state with selected node details when a node is selected.
- Modify `apps/research-radar-poc/app/page.test.tsx`
  - Assert `Research Map` is the primary post-scan artifact.
  - Assert raw source results are behind the evidence drawer.
  - Assert clicking a graph node updates the right rail.

---

### Task 1: Add Research Graph Types and Builder

**Files:**
- Modify: `apps/research-radar-poc/src/types/research.ts`
- Create: `apps/research-radar-poc/src/lib/research-graph.ts`
- Test: `apps/research-radar-poc/src/lib/research-graph.test.ts`

**Interfaces:**
- Consumes:
  - `ParsedResearchReport`
  - `ResearchSourceScanResp`
  - `ResearchSourceItem`
- Produces:
  - `ResearchGraphNodeKind`
  - `ResearchGraphNode`
  - `ResearchGraphEdge`
  - `ResearchGraph`
  - `ResearchGraphLayer`
  - `buildResearchGraph(input: BuildResearchGraphInput): ResearchGraph`
  - `parseResearchGraphBlock(markdown: string): ResearchGraph | null`
  - `nodeDetailsFor(graph: ResearchGraph, nodeId: string): ResearchGraphNode | null`

- [ ] **Step 1: Add graph type definitions to `src/types/research.ts`**

Add these exports after `ParsedResearchReport`:

```ts
export type ResearchGraphNodeKind =
  | "topic"
  | "hotspot"
  | "paper"
  | "project"
  | "model"
  | "dataset"
  | "benchmark"
  | "author"
  | "institution"
  | "open_question"
  | "experiment";

export type ResearchGraphRelation =
  | "supports"
  | "implements"
  | "evaluates"
  | "extends"
  | "reveals_gap"
  | "leads_to"
  | "mentions";

export type ResearchGraphLayer =
  | "papers"
  | "projects"
  | "models"
  | "datasets"
  | "benchmarks"
  | "questions"
  | "experiments";

export type ResearchGraphNode = {
  id: string;
  kind: ResearchGraphNodeKind;
  title: string;
  summary: string;
  importance: number;
  recency?: string | null;
  sourceItemIds: string[];
  tags: string[];
};

export type ResearchGraphEdge = {
  id: string;
  from: string;
  to: string;
  relation: ResearchGraphRelation;
  evidenceItemIds: string[];
};

export type ResearchGraph = {
  topic: string;
  nodes: ResearchGraphNode[];
  edges: ResearchGraphEdge[];
  caveats: string[];
};
```

- [ ] **Step 2: Write failing graph parser tests**

Create `apps/research-radar-poc/src/lib/research-graph.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { buildResearchGraph, nodeDetailsFor, parseResearchGraphBlock } from "./research-graph";
import type { ParsedResearchReport, ResearchSourceScanResp } from "@/types/research";

describe("research graph", () => {
  it("parses a research-graph-json fenced block", () => {
    const graph = parseResearchGraphBlock([
      "```research-graph-json",
      JSON.stringify({
        topic: "agent workflow",
        nodes: [
          {
            id: "topic:agent-workflow",
            kind: "topic",
            title: "agent workflow",
            summary: "workflow orchestration for agents",
            importance: 1,
            sourceItemIds: [],
            tags: ["agents"]
          }
        ],
        edges: [],
        caveats: ["partial source coverage"]
      }),
      "```",
      "## Research Overview",
      "Report"
    ].join("\n"));

    expect(graph?.topic).toBe("agent workflow");
    expect(graph?.nodes[0].kind).toBe("topic");
    expect(graph?.caveats).toContain("partial source coverage");
  });

  it("returns null for malformed graph JSON", () => {
    const graph = parseResearchGraphBlock([
      "```research-graph-json",
      "{ bad json",
      "```"
    ].join("\n"));

    expect(graph).toBeNull();
  });

  it("builds a deterministic source-derived graph with hotspots and caveats", () => {
    const sourceScan: ResearchSourceScanResp = {
      topic: "agent workflow",
      ranking: "balanced",
      status: "partial",
      promptContext: "",
      warnings: ["Leaderboards: endpoints are not configured"],
      sources: [
        {
          source: "arxiv",
          status: "succeeded",
          warning: null,
          items: [
            {
              id: "arxiv:1",
              source: "arxiv",
              kind: "paper",
              title: "Workflow Planning for AI Agents",
              url: "https://arxiv.org/abs/1",
              summary: "Planning and execution in multi-step agent workflows.",
              authors: ["Ada"],
              organization: null,
              publishedAt: "2026-01-01",
              updatedAt: null,
              metrics: [],
              tags: ["planning", "workflow"],
              metadata: {}
            }
          ]
        },
        {
          source: "github",
          status: "succeeded",
          warning: null,
          items: [
            {
              id: "github:agent",
              source: "github",
              kind: "project",
              title: "acme/agent-workflow",
              url: "https://github.com/acme/agent-workflow",
              summary: "Open source workflow runtime for agents.",
              authors: [],
              organization: "acme",
              publishedAt: null,
              updatedAt: "2026-02-01",
              metrics: [{ label: "stars", value: 1200 }],
              tags: ["workflow"],
              metadata: {}
            }
          ]
        }
      ],
      items: []
    };
    const parsedReport: ParsedResearchReport = {
      structured: true,
      sections: [
        {
          id: "research-openings",
          title: "Research Openings",
          content: "- Better planning reliability"
        },
        {
          id: "experiment-plans",
          title: "Experiment Plans",
          content: "- Compare workflow runtimes on long-horizon tasks"
        }
      ]
    };

    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan,
      parsedReport,
      finalOutput: ""
    });

    expect(graph.nodes.some((node) => node.kind === "topic")).toBe(true);
    expect(graph.nodes.some((node) => node.kind === "hotspot" && node.title === "workflow")).toBe(true);
    expect(graph.nodes.some((node) => node.kind === "paper")).toBe(true);
    expect(graph.nodes.some((node) => node.kind === "project")).toBe(true);
    expect(graph.nodes.some((node) => node.kind === "open_question")).toBe(true);
    expect(graph.nodes.some((node) => node.kind === "experiment")).toBe(true);
    expect(graph.edges.length).toBeGreaterThan(0);
    expect(graph.caveats).toContain("Leaderboards: endpoints are not configured");
  });

  it("returns node details by id", () => {
    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan: null,
      parsedReport: { structured: false, sections: [] },
      finalOutput: ""
    });

    expect(nodeDetailsFor(graph, "topic:agent-workflow")?.title).toBe("agent workflow");
    expect(nodeDetailsFor(graph, "missing")).toBeNull();
  });
});
```

- [ ] **Step 3: Run the failing tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/lib/research-graph.test.ts
```

Expected: fail because `./research-graph` does not exist.

- [ ] **Step 4: Implement `src/lib/research-graph.ts`**

Create `apps/research-radar-poc/src/lib/research-graph.ts` with these exported functions:

```ts
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

const GRAPH_BLOCK_PATTERN = /```research-graph-json\s*([\s\S]*?)```/i;
const MAX_HOTSPOTS = 6;
const MAX_REPORT_DERIVED_NODES = 4;

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
  const parsed = parseResearchGraphBlock(input.finalOutput);
  if (parsed) {
    return parsed;
  }

  const topicNode = topicGraphNode(input.topic);
  const sourceItems = input.sourceScan?.sources.flatMap((source) => source.items) ?? [];
  const caveats = [
    ...(input.sourceScan?.warnings ?? []),
    ...(input.sourceScan?.sources.flatMap((source) => source.warning ? [source.warning] : []) ?? [])
  ].filter(uniqueText);
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

export function nodeDetailsFor(graph: ResearchGraph, nodeId: string): ResearchGraphNode | null {
  return graph.nodes.find((node) => node.id === nodeId) ?? null;
}

function normalizeGraph(value: unknown): ResearchGraph | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const graph = value as Partial<ResearchGraph>;
  if (!graph.topic || !Array.isArray(graph.nodes) || !Array.isArray(graph.edges)) {
    return null;
  }
  return {
    topic: String(graph.topic),
    nodes: graph.nodes.map(normalizeNode).filter((node): node is ResearchGraphNode => node !== null),
    edges: graph.edges.map(normalizeEdge).filter((edge): edge is ResearchGraphEdge => edge !== null),
    caveats: Array.isArray(graph.caveats) ? graph.caveats.map(String) : []
  };
}

function normalizeNode(value: unknown): ResearchGraphNode | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const node = value as Partial<ResearchGraphNode>;
  if (!node.id || !node.kind || !node.title) {
    return null;
  }
  return {
    id: String(node.id),
    kind: node.kind,
    title: String(node.title),
    summary: node.summary ? String(node.summary) : "",
    importance: typeof node.importance === "number" ? node.importance : 0.5,
    recency: node.recency ? String(node.recency) : null,
    sourceItemIds: Array.isArray(node.sourceItemIds) ? node.sourceItemIds.map(String) : [],
    tags: Array.isArray(node.tags) ? node.tags.map(String) : []
  };
}

function normalizeEdge(value: unknown): ResearchGraphEdge | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const edge = value as Partial<ResearchGraphEdge>;
  if (!edge.id || !edge.from || !edge.to || !edge.relation) {
    return null;
  }
  return {
    id: String(edge.id),
    from: String(edge.from),
    to: String(edge.to),
    relation: edge.relation,
    evidenceItemIds: Array.isArray(edge.evidenceItemIds) ? edge.evidenceItemIds.map(String) : []
  };
}

function topicGraphNode(topic: string): ResearchGraphNode {
  return {
    id: `topic:${slug(topic)}`,
    kind: "topic",
    title: topic.trim() || "Research Topic",
    summary: "Central research point for this radar scan.",
    importance: 1,
    sourceItemIds: [],
    tags: []
  };
}

function buildHotspotNodes(topic: string, items: ResearchSourceItem[]): ResearchGraphNode[] {
  const counts = new Map<string, number>();
  items.forEach((item) => {
    const terms = [...item.tags, ...titleTerms(item.title)].filter((term) => term !== slug(topic));
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
      summary: kind === "experiment" ? "Candidate experiment plan from the analysis report." : "Open research question from the analysis report.",
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
  return value.toLowerCase().trim().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

function uniqueText(value: string, index: number, values: string[]) {
  return value.trim().length > 0 && values.indexOf(value) === index;
}
```

- [ ] **Step 5: Run graph tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/lib/research-graph.test.ts
```

Expected: all 4 tests pass.

- [ ] **Step 6: Commit Task 1**

```bash
git add apps/research-radar-poc/src/types/research.ts apps/research-radar-poc/src/lib/research-graph.ts apps/research-radar-poc/src/lib/research-graph.test.ts
git commit -m "feat: add research radar graph builder"
```

---

### Task 2: Update Agent Prompt for Graph JSON

**Files:**
- Modify: `apps/research-radar-poc/src/api/research.ts`
- Modify: `apps/research-radar-poc/src/api/research.test.ts`

**Interfaces:**
- Consumes:
  - `buildResearchRadarAgentRunCommand(input: ResearchScanInput): AgentRunCommand`
- Produces:
  - Agent prompt that asks for `research-graph-json` plus existing report headings.

- [ ] **Step 1: Add failing prompt tests**

In `apps/research-radar-poc/src/api/research.test.ts`, add:

```ts
  it("asks the Agent for graph JSON before the markdown report", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "agent workflow",
      filters: ["papers", "projects"],
      ranking: "balanced",
      routeId: "runtime.llm"
    });

    expect(command.input).toContain("```research-graph-json");
    expect(command.input).toContain('"nodes"');
    expect(command.input).toContain('"edges"');
    expect(command.input.indexOf("```research-graph-json")).toBeLessThan(
      command.input.indexOf("## Research Overview")
    );
    expect(command.input.length).toBeLessThanOrEqual(4000);
  });
```

- [ ] **Step 2: Run the failing prompt test**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/api/research.test.ts
```

Expected: fail because the prompt does not include `research-graph-json`.

- [ ] **Step 3: Add graph instruction text to `src/api/research.ts`**

Add this constant near `REPORT_HEADINGS`:

```ts
const GRAPH_JSON_INSTRUCTION = [
  "Before the markdown report, return one compact fenced graph block:",
  "```research-graph-json",
  '{ "topic": "...", "nodes": [], "edges": [], "caveats": [] }',
  "```",
  "Graph node kinds: topic, hotspot, paper, project, model, dataset, benchmark, author, institution, open_question, experiment.",
  "Graph edge relations: supports, implements, evaluates, extends, reveals_gap, leads_to, mentions.",
  "Keep graph JSON compact: at most 18 nodes and 28 edges."
];
```

Then change `afterEvidence` in `buildResearchRadarPrompt` so the graph instruction appears before report headings:

```ts
  const afterEvidence = [
    "Use web search when useful. Prefer recent, source-grounded information, but clearly mark uncertainty, stale information, and missing coverage.",
    "Use at most 3 web search calls total. After those searches, synthesize the report with caveats instead of searching again.",
    ...GRAPH_JSON_INSTRUCTION,
    "After the graph block, return a concise markdown report with exactly these headings:",
    ...REPORT_HEADINGS,
    "For each section, include practical details that help a newcomer decide what to read, who to follow, what work matters, and which experiments are worth trying."
  ];
```

- [ ] **Step 4: Run prompt tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/api/research.test.ts
```

Expected: all tests pass, including the existing 4000 character limit test.

- [ ] **Step 5: Commit Task 2**

```bash
git add apps/research-radar-poc/src/api/research.ts apps/research-radar-poc/src/api/research.test.ts
git commit -m "feat: request research graph from radar agent"
```

---

### Task 3: Build the Research Map Component

**Files:**
- Create: `apps/research-radar-poc/src/components/research-map.tsx`
- Test: `apps/research-radar-poc/src/components/research-map.test.tsx`

**Interfaces:**
- Consumes:
  - `ResearchGraph`
  - `ResearchGraphLayer`
- Produces:
  - `ResearchMap({ graph, selectedNodeId, onNodeSelect }: ResearchMapProps)`
  - Layer toggles and deterministic SVG node layout.

- [ ] **Step 1: Write failing component tests**

Create `apps/research-radar-poc/src/components/research-map.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ResearchMap } from "./research-map";
import type { ResearchGraph } from "@/types/research";

const graph: ResearchGraph = {
  topic: "agent workflow",
  caveats: ["partial coverage"],
  nodes: [
    {
      id: "topic:agent-workflow",
      kind: "topic",
      title: "agent workflow",
      summary: "central topic",
      importance: 1,
      sourceItemIds: [],
      tags: []
    },
    {
      id: "hotspot:planning",
      kind: "hotspot",
      title: "planning",
      summary: "recurring hotspot",
      importance: 0.8,
      sourceItemIds: [],
      tags: ["planning"]
    },
    {
      id: "source:arxiv:1",
      kind: "paper",
      title: "Workflow Planning for AI Agents",
      summary: "paper summary",
      importance: 0.6,
      sourceItemIds: ["arxiv:1"],
      tags: ["planning"]
    },
    {
      id: "experiment:compare-runtimes",
      kind: "experiment",
      title: "Compare workflow runtimes",
      summary: "candidate experiment",
      importance: 0.7,
      sourceItemIds: [],
      tags: []
    }
  ],
  edges: [
    {
      id: "topic:agent-workflow->hotspot:planning:mentions",
      from: "topic:agent-workflow",
      to: "hotspot:planning",
      relation: "mentions",
      evidenceItemIds: []
    },
    {
      id: "hotspot:planning->source:arxiv:1:supports",
      from: "hotspot:planning",
      to: "source:arxiv:1",
      relation: "supports",
      evidenceItemIds: ["arxiv:1"]
    }
  ]
};

describe("ResearchMap", () => {
  it("renders a research map with nodes and relations", () => {
    render(<ResearchMap graph={graph} selectedNodeId={null} onNodeSelect={() => {}} />);

    expect(screen.getByText("Research Map")).toBeTruthy();
    expect(screen.getByRole("button", { name: /agent workflow/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /planning/ })).toBeTruthy();
    expect(screen.getByText("supports")).toBeTruthy();
  });

  it("selects a node when clicked", () => {
    const onNodeSelect = vi.fn();
    render(<ResearchMap graph={graph} selectedNodeId={null} onNodeSelect={onNodeSelect} />);

    fireEvent.click(screen.getByRole("button", { name: /Workflow Planning for AI Agents/ }));

    expect(onNodeSelect).toHaveBeenCalledWith("source:arxiv:1");
  });

  it("hides paper nodes when the Papers layer is disabled", () => {
    render(<ResearchMap graph={graph} selectedNodeId={null} onNodeSelect={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "Papers" }));

    expect(screen.queryByRole("button", { name: /Workflow Planning for AI Agents/ })).toBeNull();
    expect(screen.getByRole("button", { name: /agent workflow/ })).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run the failing component tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/components/research-map.test.tsx
```

Expected: fail because `src/components/research-map.tsx` does not exist.

- [ ] **Step 3: Implement `ResearchMap`**

Create `apps/research-radar-poc/src/components/research-map.tsx`:

```tsx
"use client";

import { useMemo, useState } from "react";
import { ArrowUpRight, Beaker, BookOpen, Boxes, Database, GitBranch, HelpCircle, Lightbulb, Network, Orbit, Package, Users } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { ResearchGraph, ResearchGraphLayer, ResearchGraphNode, ResearchGraphNodeKind } from "@/types/research";

type ResearchMapProps = {
  graph: ResearchGraph;
  selectedNodeId: string | null;
  onNodeSelect: (nodeId: string) => void;
};

const LAYERS: Array<{ layer: ResearchGraphLayer; label: string; kinds: ResearchGraphNodeKind[] }> = [
  { layer: "papers", label: "Papers", kinds: ["paper"] },
  { layer: "projects", label: "Projects", kinds: ["project"] },
  { layer: "models", label: "Models", kinds: ["model"] },
  { layer: "datasets", label: "Datasets", kinds: ["dataset"] },
  { layer: "benchmarks", label: "Benchmarks", kinds: ["benchmark"] },
  { layer: "questions", label: "Questions", kinds: ["open_question"] },
  { layer: "experiments", label: "Experiments", kinds: ["experiment"] }
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

export function ResearchMap({ graph, selectedNodeId, onNodeSelect }: ResearchMapProps) {
  const [enabledLayers, setEnabledLayers] = useState<Set<ResearchGraphLayer>>(
    () => new Set(LAYERS.map((layer) => layer.layer))
  );
  const positioned = useMemo(() => layoutGraph(graph.nodes), [graph.nodes]);
  const visibleNodes = positioned.filter((node) => node.kind === "topic" || node.kind === "hotspot" || nodeVisibleForLayers(node, enabledLayers));
  const visibleIds = new Set(visibleNodes.map((node) => node.id));
  const visibleEdges = graph.edges.filter((edge) => visibleIds.has(edge.from) && visibleIds.has(edge.to));

  return (
    <section className="rounded-[8px] border border-[#DEE6DE] bg-white p-5 shadow-[0_10px_24px_rgba(34,45,38,0.05)]">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h3 className="flex items-center gap-2 text-[16px] font-semibold text-[#17251F]">
            <Network aria-hidden="true" className="h-4 w-4 text-[#0E6B5F]" strokeWidth={1.9} />
            Research Map
          </h3>
          <p className="mt-1 text-[12px] text-[#6B776F]">
            Explore how topics, evidence, gaps, and experiments connect.
          </p>
        </div>
        <span className="rounded-[7px] bg-[#EEF3ED] px-2 py-1 text-[12px] text-[#66736B]">
          {graph.nodes.length} nodes
        </span>
      </div>

      <div className="mt-4 flex flex-wrap gap-2">
        {LAYERS.map((layer) => {
          const active = enabledLayers.has(layer.layer);
          return (
            <button
              aria-pressed={active}
              className={[
                "h-8 rounded-[8px] border px-2.5 text-[12px] font-medium transition",
                active ? "border-[#0E6B5F] bg-[#E9F7F3] text-[#0B5D53]" : "border-[#DDE5DD] bg-white text-[#66736B]"
              ].join(" ")}
              key={layer.layer}
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
              type="button"
            >
              {layer.label}
            </button>
          );
        })}
      </div>

      <div className="relative mt-4 h-[520px] overflow-hidden rounded-[8px] border border-[#E5ECE5] bg-[#FBFCFA]">
        <svg aria-hidden="true" className="absolute inset-0 h-full w-full" viewBox="0 0 1000 520">
          {visibleEdges.map((edge) => {
            const from = positioned.find((node) => node.id === edge.from);
            const to = positioned.find((node) => node.id === edge.to);
            if (!from || !to) {
              return null;
            }
            return (
              <g key={edge.id}>
                <line x1={from.x} x2={to.x} y1={from.y} y2={to.y} stroke="#CBD7D1" strokeWidth="1.5" />
                <text fill="#7A857E" fontSize="11" textAnchor="middle" x={(from.x + to.x) / 2} y={(from.y + to.y) / 2 - 4}>
                  {edge.relation}
                </text>
              </g>
            );
          })}
        </svg>

        {visibleNodes.map((node) => {
          const Icon = KIND_ICON[node.kind];
          const selected = node.id === selectedNodeId;
          return (
            <button
              aria-label={`${node.kind}: ${node.title}`}
              className={[
                "absolute flex max-w-[190px] -translate-x-1/2 -translate-y-1/2 items-center gap-2 rounded-[8px] border px-3 py-2 text-left shadow-sm transition",
                nodeTone(node.kind),
                selected ? "ring-2 ring-[#0E6B5F] ring-offset-2" : "hover:border-[#0E6B5F]"
              ].join(" ")}
              key={node.id}
              onClick={() => onNodeSelect(node.id)}
              style={{ left: `${node.x / 10}%`, top: `${node.y / 5.2}%` }}
              title={node.summary}
              type="button"
            >
              <Icon aria-hidden="true" className="h-4 w-4 shrink-0" strokeWidth={1.9} />
              <span className="min-w-0">
                <span className="block truncate text-[12px] font-semibold">{node.title}</span>
                <span className="block truncate text-[10px] opacity-75">{node.kind}</span>
              </span>
              {node.sourceItemIds.length > 0 ? <ArrowUpRight aria-hidden="true" className="h-3 w-3 shrink-0 opacity-60" /> : null}
            </button>
          );
        })}
      </div>
    </section>
  );
}

type PositionedNode = ResearchGraphNode & { x: number; y: number };

function layoutGraph(nodes: ResearchGraphNode[]): PositionedNode[] {
  const topic = nodes.find((node) => node.kind === "topic");
  const others = nodes.filter((node) => node.kind !== "topic");
  const radius = 190;
  const center = { x: 500, y: 260 };
  const positioned: PositionedNode[] = topic ? [{ ...topic, ...center }] : [];
  others.forEach((node, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(1, others.length) - Math.PI / 2;
    const layerOffset = node.kind === "hotspot" ? -55 : 35;
    const nodeRadius = radius + layerOffset;
    positioned.push({
      ...node,
      x: Math.round(center.x + Math.cos(angle) * nodeRadius),
      y: Math.round(center.y + Math.sin(angle) * nodeRadius)
    });
  });
  return positioned;
}

function nodeVisibleForLayers(node: ResearchGraphNode, enabledLayers: Set<ResearchGraphLayer>) {
  return LAYERS.some((layer) => layer.kinds.includes(node.kind) && enabledLayers.has(layer.layer));
}

function nodeTone(kind: ResearchGraphNodeKind) {
  if (kind === "topic") {
    return "border-[#0E6B5F] bg-[#0E6B5F] text-white";
  }
  if (kind === "hotspot") {
    return "border-[#BFD8F3] bg-[#F3F8FF] text-[#1F4F7A]";
  }
  if (kind === "open_question") {
    return "border-[#F5D2C8] bg-[#FFF6F3] text-[#9B3C2C]";
  }
  if (kind === "experiment") {
    return "border-[#F3D7A6] bg-[#FFF8E8] text-[#8A5A10]";
  }
  return "border-[#DDE8E3] bg-white text-[#17251F]";
}
```

- [ ] **Step 4: Run component tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/components/research-map.test.tsx
```

Expected: all 3 tests pass.

- [ ] **Step 5: Commit Task 3**

```bash
git add apps/research-radar-poc/src/components/research-map.tsx apps/research-radar-poc/src/components/research-map.test.tsx
git commit -m "feat: add research radar graph map"
```

---

### Task 4: Integrate Graph-First Workspace and Inspector

**Files:**
- Modify: `apps/research-radar-poc/src/app-client.tsx`
- Modify: `apps/research-radar-poc/app/page.test.tsx`

**Interfaces:**
- Consumes:
  - `buildResearchGraph(input: BuildResearchGraphInput): ResearchGraph`
  - `nodeDetailsFor(graph: ResearchGraph, nodeId: string): ResearchGraphNode | null`
  - `ResearchMap`
- Produces:
  - A graph-first center workspace.
  - A selected-node inspector in the right rail.
  - A collapsible raw evidence drawer.

- [ ] **Step 1: Add failing page tests for graph-first behavior**

Modify `apps/research-radar-poc/app/page.test.tsx`:

1. In the existing `"runs backend source scan before creating the Agent run"` test, replace:

```ts
    expect(await screen.findByText("Source Results")).toBeTruthy();
    expect(await screen.findByText("acme/agent")).toBeTruthy();
```

with:

```ts
    expect(await screen.findByText("Research Map")).toBeTruthy();
    expect(await screen.findByRole("button", { name: /acme\/agent/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Evidence Drawer" })).toBeTruthy();
```

2. Add a new test:

```ts
  it("updates the right rail when a graph node is selected", async () => {
    const fetchMock = vi.fn(async (url: string) => {
      const href = String(url);
      if (href.includes("/ai/research-radar/scans")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              topic: "AI coding agents",
              ranking: "balanced",
              status: "succeeded",
              warnings: [],
              promptContext: "Research Radar Evidence\n[github] Project: acme/agent",
              sources: [
                {
                  source: "github",
                  status: "succeeded",
                  warning: null,
                  items: [
                    {
                      id: "github:acme/agent",
                      source: "github",
                      kind: "project",
                      title: "acme/agent",
                      url: "https://github.com/acme/agent",
                      summary: "Agent workflow runtime",
                      authors: [],
                      organization: "acme",
                      publishedAt: null,
                      updatedAt: "2026-06-01T00:00:00Z",
                      metrics: [{ label: "stars", value: 1200 }],
                      tags: ["workflow"],
                      metadata: {}
                    }
                  ]
                }
              ],
              items: []
            }
          })
        };
      }
      if (href.includes("/ai/agents/runs") && !href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              runId: 92,
              traceId: "agent-92",
              status: "succeeded",
              finalOutput: [
                "```research-graph-json",
                JSON.stringify({
                  topic: "AI coding agents",
                  nodes: [
                    {
                      id: "topic:ai-coding-agents",
                      kind: "topic",
                      title: "AI coding agents",
                      summary: "Coding-focused agent systems",
                      importance: 1,
                      sourceItemIds: [],
                      tags: []
                    },
                    {
                      id: "source:github:acme/agent",
                      kind: "project",
                      title: "acme/agent",
                      summary: "Agent workflow runtime",
                      importance: 0.8,
                      sourceItemIds: ["github:acme/agent"],
                      tags: ["workflow"]
                    }
                  ],
                  edges: [
                    {
                      id: "topic:ai-coding-agents->source:github:acme/agent:implements",
                      from: "topic:ai-coding-agents",
                      to: "source:github:acme/agent",
                      relation: "implements",
                      evidenceItemIds: ["github:acme/agent"]
                    }
                  ],
                  caveats: []
                }),
                "```",
                "## Research Overview",
                "Report",
                "## Active Topics",
                "Workflow reliability",
                "## Key Authors And Institutions",
                "acme",
                "## Representative Work",
                "acme/agent",
                "## Reading Route",
                "Start with workflow runtimes.",
                "## Research Openings",
                "- Better planning reliability",
                "## Experiment Plans",
                "- Compare workflow runtimes",
                "## Sources And Caveats",
                "Source coverage is partial."
              ].join("\n")
            }
          })
        };
      }
      if (href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({ code: "200", data: { list: [], total: 0 } })
        };
      }
      return { ok: true, json: async () => ({ code: "200", data: {} }) };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.change(screen.getByLabelText("研究主题"), {
      target: { value: "AI coding agents" }
    });
    fireEvent.click(screen.getByRole("button", { name: "启动雷达扫描" }));
    fireEvent.click(await screen.findByRole("button", { name: /acme\/agent/ }));

    expect(await screen.findByText("Node Inspector")).toBeTruthy();
    expect(await screen.findByText("Agent workflow runtime")).toBeTruthy();
    expect(await screen.findByText("project")).toBeTruthy();
  });
```

- [ ] **Step 2: Run failing page tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx
```

Expected: fail because `Research Map`, `Evidence Drawer`, and node inspector are not integrated.

- [ ] **Step 3: Integrate graph state in `src/app-client.tsx`**

Add imports near existing imports:

```ts
import { ResearchMap } from "@/components/research-map";
import { buildResearchGraph, nodeDetailsFor } from "@/lib/research-graph";
import type { ResearchGraph, ResearchGraphNode } from "@/types/research";
```

Add state near `activeScanId`:

```ts
  const [selectedGraphNodeId, setSelectedGraphNodeId] = useState<string | null>(null);
```

Add memoized graph:

```ts
  const researchGraph = useMemo(
    () =>
      activeScan
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
```

When a new scan starts, clear selection after `setActiveScanId(scanId);`:

```ts
    setSelectedGraphNodeId(null);
```

Pass graph props:

```tsx
            <ReportWorkspace
              activeScan={activeScan}
              isSubmitting={isSubmitting}
              onGraphNodeSelect={setSelectedGraphNodeId}
              parsedReport={parsedReport}
              researchGraph={researchGraph}
              selectedGraphNodeId={selectedGraphNodeId}
            />
```

Pass inspector props:

```tsx
        <EvidenceRail
          activeScan={activeScan}
          eventEvidence={eventEvidence}
          modelDeltaSummary={modelDeltaSummary}
          researchGraph={researchGraph}
          selectedGraphNode={selectedGraphNode}
        />
```

- [ ] **Step 4: Make `ReportWorkspace` graph-first**

Change its props:

```ts
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
```

Replace the current inline `SourceResults` placement:

```tsx
      {activeScan.sourceScan ? <SourceResults sources={activeScan.sourceScan.sources} /> : null}
```

with:

```tsx
      {researchGraph ? (
        <ResearchMap
          graph={researchGraph}
          onNodeSelect={onGraphNodeSelect}
          selectedNodeId={selectedGraphNodeId}
        />
      ) : null}
```

Move raw sources below report sections inside a collapsible drawer:

```tsx
      {activeScan.sourceScan ? <EvidenceDrawer sources={activeScan.sourceScan.sources} /> : null}
```

Create this component in `app-client.tsx` above `SourceResults`:

```tsx
function EvidenceDrawer({ sources }: { sources: ResearchSourceResult[] }) {
  const [open, setOpen] = useState(false);
  return (
    <section className="rounded-[8px] border border-[#DEE6DE] bg-white p-4">
      <button
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
```

- [ ] **Step 5: Update `EvidenceRail` into a node inspector**

Change its props:

```ts
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
```

At the top of the rail content, replace the old waiting-only block with:

```tsx
        {selectedGraphNode ? (
          <section className="mb-4 rounded-[8px] border border-[#D7E7FF] bg-[#F8FBFF] p-3">
            <h2 className="text-[14px] font-semibold text-[#1D2B39]">Node Inspector</h2>
            <div className="mt-2 flex flex-wrap gap-1.5">
              <span className="rounded-[7px] bg-white px-2 py-0.5 text-[11px] text-[#53687F]">
                {selectedGraphNode.kind}
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
```

Remove the old duplicate `activeScan?.runResult` block so the rail has one primary state.

- [ ] **Step 6: Run page tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx
```

Expected: all page tests pass.

- [ ] **Step 7: Commit Task 4**

```bash
git add apps/research-radar-poc/src/app-client.tsx apps/research-radar-poc/app/page.test.tsx
git commit -m "feat: make research radar graph first"
```

---

### Task 5: Final Verification

**Files:**
- No new files.

**Interfaces:**
- Consumes:
  - All tasks above.
- Produces:
  - Verified graph-first Research Radar POC.

- [ ] **Step 1: Run the full POC test set**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx src/api/research.test.ts src/api/source-scan.test.ts src/lib/research-graph.test.ts src/components/research-map.test.tsx
```

Expected: all tests pass.

- [ ] **Step 2: Run typecheck**

Run:

```bash
pnpm --dir apps/research-radar-poc typecheck
```

Expected: `tsc --noEmit` exits 0.

- [ ] **Step 3: Run lint**

Run:

```bash
pnpm --dir apps/research-radar-poc lint
```

Expected: `eslint .` exits 0.

- [ ] **Step 4: Run production build**

Run:

```bash
pnpm --dir apps/research-radar-poc build
```

Expected: Next build exits 0. If `apps/research-radar-poc/next-env.d.ts` changes due to Next generation, restore only that generated change with `apply_patch`.

- [ ] **Step 5: Run whitespace check**

Run:

```bash
git diff --check
```

Expected: no output and exit 0.

- [ ] **Step 6: Live smoke**

Ensure backend and frontend are available:

```bash
curl -sS http://127.0.0.1:62601/ready
curl -sS -I http://127.0.0.1:62607 | head -n 5
```

Expected:

- backend returns `{"code":"200","data":"ready",...}`
- frontend returns `HTTP/1.1 200 OK`

- [ ] **Step 7: Final status check**

Run:

```bash
git status --short
```

Expected: only intentional changes remain; do not modify, stage, or revert `apps/codex-app-poc/app/layout.tsx`.

