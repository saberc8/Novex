# Research Radar Graph Analysis Design

Date: 2026-06-25

## Goal

Upgrade the Research Radar POC from a source-result list into a graph-first research analysis tool.

The user enters one research point, such as `agent workflow`. The system should help them understand the research landscape around that point: active subproblems, representative work, code/models, datasets, benchmarks, people/institutions, unresolved questions, reading order, and experiment ideas.

The main screen should make the relationship between evidence and insights visible. Raw sources remain available, but they should no longer be the primary display.

## Current Problem

The current POC technically collects relevant evidence from arXiv, GitHub, HuggingFace, PapersWithCode-compatible endpoints, and leaderboard endpoints. It also asks an Agent to synthesize a report.

The product problem is that the first thing users see is still a grouped list of source items. This reads like a search result page, not like an "AI research radar". A newcomer cannot quickly answer:

- What are the main subtopics inside this research point?
- Which papers/projects/datasets support each subtopic?
- What is important versus merely related?
- What should be read first?
- Where are the gaps and possible experiments?

## Product Direction

Use a graph-first layout.

The central node is the user query. Around it, the system generates analysis nodes:

- Hotspots: active research subproblems or themes.
- Papers: representative or recent papers.
- Projects and Models: GitHub projects and HuggingFace models.
- Datasets: relevant datasets and traces.
- Benchmarks: evaluation suites and leaderboard signals.
- Authors and Institutions: recurring people/labs when available.
- Open Questions: contradictions, missing evidence, unverified assumptions.
- Experiment Ideas: possible next experiments or research cuts.

Edges should express why two nodes are connected:

- `supports`: a paper supports a hotspot.
- `implements`: a project/model implements a method.
- `evaluates`: a dataset/benchmark evaluates a capability.
- `extends`: one work builds on another.
- `reveals_gap`: a source points to an open question.
- `leads_to`: an open question leads to an experiment idea.

The graph is not decorative. It is the main navigation model for the analysis.

## Target Experience

After a scan:

1. The user sees a research map, centered on the query.
2. Important hotspots appear as prominent nodes.
3. Evidence nodes cluster around hotspots by source type.
4. The user can select a node to inspect details in the right rail.
5. The selected node shows:
   - Why it matters.
   - Connected evidence.
   - Source URLs and caveats.
   - Suggested next action.
6. Below the graph, the user sees compact analysis sections:
   - Reading Roadmap.
   - Research Openings.
   - Experiment Plans.
   - Sources and Caveats.
7. Raw Source Results move into a collapsible Evidence Drawer.

## Layout

Keep the current three-column shell:

- Left rail: scan history.
- Center: composer plus graph-first analysis workspace.
- Right rail: selected node inspector and evidence details.

Center workspace order:

1. Scan summary strip: topic, status, source count, graph node count, caveat count.
2. Research Map: interactive graph canvas or SVG/HTML graph.
3. Insight bands: Reading Roadmap, Open Questions, Experiment Plans.
4. Collapsible Evidence Drawer: raw source groups.

Right rail:

1. Empty state: "Select a node".
2. Node selected:
   - Node title and type.
   - Importance / recency / evidence count.
   - Explanation.
   - Connected nodes.
   - Source links.
   - Caveats.

## Data Model

Add a frontend graph model derived from existing scan data and Agent report output:

```ts
type ResearchGraphNodeKind =
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

type ResearchGraphNode = {
  id: string;
  kind: ResearchGraphNodeKind;
  title: string;
  summary: string;
  importance: number;
  recency?: string | null;
  sourceItemIds: string[];
  tags: string[];
};

type ResearchGraphEdge = {
  id: string;
  from: string;
  to: string;
  relation:
    | "supports"
    | "implements"
    | "evaluates"
    | "extends"
    | "reveals_gap"
    | "leads_to"
    | "mentions";
  evidenceItemIds: string[];
};

type ResearchGraph = {
  topic: string;
  nodes: ResearchGraphNode[];
  edges: ResearchGraphEdge[];
  caveats: string[];
};
```

For the POC, graph construction can happen in the frontend from two inputs:

- `sourceScan.items` and `sourceScan.sources`.
- Parsed Agent report sections.

The Agent prompt should ask for a compact machine-readable graph block before the markdown report. The frontend parser should prefer that graph block when present and fall back to deterministic source-based graph construction when the model output is missing or malformed.

## Graph Construction Rules

Fallback graph generation should be deterministic and testable:

1. Always create one `topic` node.
2. Create evidence nodes from source items:
   - `paper` from arXiv/PapersWithCode.
   - `project` from GitHub.
   - `model` from HuggingFace models.
   - `dataset` from HuggingFace datasets.
   - `benchmark` from leaderboards/PapersWithCode benchmark-like items.
3. Create hotspot nodes by extracting recurring tags, title terms, and report section bullets.
4. Link each evidence node to the strongest matching hotspot.
5. If no hotspot can be inferred, link evidence directly to the topic.
6. Create open question and experiment nodes from parsed report sections when available.
7. Preserve warnings as graph caveats rather than hiding them.

The graph must degrade gracefully. If the Agent run fails, the user should still get a source-derived map.

## Visual Design

The UI should feel like a working research tool, not a marketing page.

Graph style:

- Topic node: largest, dark green.
- Hotspot nodes: medium, blue or teal.
- Evidence nodes: smaller, color-coded by source type.
- Gap/experiment nodes: amber and red accents.
- Edges: thin neutral lines with relation labels on hover/selection.

Interaction:

- Click node: update right rail inspector.
- Hover node: show title and kind.
- Click source link: open original URL.
- Toggle layers: Papers, Projects/Models, Datasets, Benchmarks, Questions, Experiments.
- Fit graph in viewport; no overlap with composer or report sections.

The graph can be implemented with SVG and React state for the POC. A force-directed library is optional, but hand-rolled graph layout is acceptable if it stays deterministic, readable, and testable.

## Prompt Changes

The Agent prompt should explicitly ask for two outputs:

1. A compact JSON graph block:

````md
```research-graph-json
{ "nodes": [], "edges": [], "caveats": [] }
```
````

2. The existing markdown report headings.

The frontend should keep the total Agent input within 4000 characters. Source evidence truncation must remain in place.

## Error Handling

- If source scan fails completely, show the existing failure alert.
- If source scan partially succeeds, render a partial graph and show caveats.
- If Agent run fails, render source-derived graph and show "model analysis unavailable".
- If graph JSON parsing fails, fall back to source-derived graph.
- If there are no usable nodes, show a clear empty state with source warnings.

## Testing

Add focused tests for:

- Graph parser accepts valid `research-graph-json`.
- Graph parser falls back safely on malformed JSON.
- Source-derived graph creates topic, evidence, hotspot, and caveat nodes.
- Page renders `Research Map` before raw source results.
- Clicking a graph node updates the inspector.
- Agent prompt includes graph JSON instructions while staying within 4000 characters.

Existing tests for source scan, Agent input limit, and failed-source behavior should remain.

## Out Of Scope

- Persisting graph data to the backend.
- Collaborative editing.
- Real-time graph animation.
- Full citation ranking model.
- Building a production knowledge graph database.

The POC should prove the workflow and display model first.
