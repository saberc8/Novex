import { describe, expect, it } from "vitest";
import { buildResearchGraph, nodeDetailsFor, parseResearchGraphBlock } from "./research-graph";
import type { ParsedResearchReport, ResearchSourceScanResp } from "@/types/research";

describe("research graph", () => {
  it("parses a research-graph-json fenced block", () => {
    const graph = parseResearchGraphBlock(
      [
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
      ].join("\n")
    );

    expect(graph?.topic).toBe("agent workflow");
    expect(graph?.nodes[0].kind).toBe("topic");
    expect(graph?.caveats).toContain("partial source coverage");
  });

  it("returns null for malformed graph JSON", () => {
    const graph = parseResearchGraphBlock(
      [
        "```research-graph-json",
        "{ bad json",
        "```"
      ].join("\n")
    );

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

  it("uses both top-level source items and grouped source items while deduping by id", () => {
    const sourceScan: ResearchSourceScanResp = {
      topic: "agent workflow",
      ranking: "balanced",
      status: "partial",
      promptContext: "",
      warnings: [],
      sources: [
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
        },
        {
          id: "leaderboards:swe-bench",
          source: "leaderboards",
          kind: "benchmark",
          title: "SWE-bench Verified",
          url: "https://leaderboard.example/swe-bench",
          summary: "Benchmark for repository-scale coding tasks.",
          authors: [],
          organization: null,
          publishedAt: null,
          updatedAt: "2026-03-01",
          metrics: [{ label: "score", value: 73 }],
          tags: ["evaluation", "workflow"],
          metadata: {}
        }
      ]
    };

    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan,
      parsedReport: { structured: false, sections: [] },
      finalOutput: ""
    });

    expect(graph.nodes.filter((node) => node.id === "source:github:agent")).toHaveLength(1);
    expect(graph.nodes.some((node) => node.id === "source:leaderboards:swe-bench")).toBe(true);
  });

  it("preserves source caveats when model graph JSON parses successfully", () => {
    const sourceScan: ResearchSourceScanResp = {
      topic: "agent workflow",
      ranking: "balanced",
      status: "partial",
      promptContext: "",
      warnings: ["Leaderboards: endpoints are not configured"],
      sources: [
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

    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan,
      parsedReport: { structured: false, sections: [] },
      finalOutput: [
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
              tags: []
            },
            {
              id: "source:github:agent",
              kind: "project",
              title: "acme/agent-workflow",
              summary: "Open source workflow runtime for agents.",
              importance: 0.8,
              sourceItemIds: ["github:agent"],
              tags: ["workflow"]
            }
          ],
          edges: [
            {
              id: "topic:agent-workflow->source:github:agent:implements",
              from: "topic:agent-workflow",
              to: "source:github:agent",
              relation: "implements",
              evidenceItemIds: ["github:agent"]
            }
          ],
          caveats: ["model caveat"]
        }),
        "```"
      ].join("\n")
    });

    expect(graph.caveats).toContain("model caveat");
    expect(graph.caveats).toContain("Leaderboards: endpoints are not configured");
  });

  it("falls back to a source-derived graph when model graph JSON has no usable nodes", () => {
    const sourceScan: ResearchSourceScanResp = {
      topic: "agent workflow",
      ranking: "balanced",
      status: "succeeded",
      promptContext: "",
      warnings: [],
      sources: [
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

    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan,
      parsedReport: { structured: false, sections: [] },
      finalOutput: [
        "```research-graph-json",
        JSON.stringify({
          topic: "agent workflow",
          nodes: [],
          edges: [],
          caveats: []
        }),
        "```"
      ].join("\n")
    });

    expect(graph.nodes.some((node) => node.kind === "project" && node.title === "acme/agent-workflow")).toBe(true);
  });

  it("repairs a parsed graph that omits the topic node by re-centering it on the source-derived topic", () => {
    const sourceScan: ResearchSourceScanResp = {
      topic: "agent workflow",
      ranking: "balanced",
      status: "succeeded",
      promptContext: "",
      warnings: [],
      sources: [
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

    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan,
      parsedReport: { structured: false, sections: [] },
      finalOutput: [
        "```research-graph-json",
        JSON.stringify({
          topic: "agent workflow",
          nodes: [
            {
              id: "hotspot:workflow",
              kind: "hotspot",
              title: "workflow",
              summary: "Core orchestration theme.",
              importance: 0.9,
              sourceItemIds: ["github:agent"],
              tags: ["workflow"]
            },
            {
              id: "source:github:agent",
              kind: "project",
              title: "acme/agent-workflow",
              summary: "Open source workflow runtime for agents.",
              importance: 0.8,
              sourceItemIds: ["github:agent"],
              tags: ["workflow"]
            }
          ],
          edges: [
            {
              id: "hotspot:workflow->source:github:agent:implements",
              from: "hotspot:workflow",
              to: "source:github:agent",
              relation: "implements",
              evidenceItemIds: ["github:agent"]
            }
          ],
          caveats: []
        }),
        "```"
      ].join("\n")
    });

    expect(graph.nodes.some((node) => node.kind === "topic" && node.title === "agent workflow")).toBe(true);
    expect(graph.edges.some((edge) => edge.from === "topic:agent-workflow" && edge.to === "hotspot:workflow")).toBe(true);
  });

  it("falls back when parsed graph edge endpoints cannot be repaired", () => {
    const sourceScan: ResearchSourceScanResp = {
      topic: "agent workflow",
      ranking: "balanced",
      status: "succeeded",
      promptContext: "",
      warnings: [],
      sources: [
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

    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan,
      parsedReport: { structured: false, sections: [] },
      finalOutput: [
        "```research-graph-json",
        JSON.stringify({
          topic: "agent workflow",
          nodes: [
            {
              id: "source:github:agent",
              kind: "project",
              title: "acme/agent-workflow",
              summary: "Open source workflow runtime for agents.",
              importance: 0.8,
              sourceItemIds: ["github:agent"],
              tags: ["workflow"]
            }
          ],
          edges: [
            {
              id: "missing:hotspot->source:github:agent:implements",
              from: "missing:hotspot",
              to: "source:github:agent",
              relation: "implements",
              evidenceItemIds: ["github:agent"]
            }
          ],
          caveats: []
        }),
        "```"
      ].join("\n")
    });

    expect(graph.nodes.some((node) => node.kind === "hotspot")).toBe(true);
    expect(graph.nodes.some((node) => node.kind === "topic")).toBe(true);
    expect(graph.edges.some((edge) => edge.from === "topic:agent-workflow")).toBe(true);
  });

  it("returns node details with connected evidence, sources, caveats, and next action", () => {
    const sourceScan: ResearchSourceScanResp = {
      topic: "agent workflow",
      ranking: "balanced",
      status: "partial",
      promptContext: "",
      warnings: ["coverage is partial"],
      sources: [
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

    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan,
      parsedReport: { structured: false, sections: [] },
      finalOutput: [
        "```research-graph-json",
        JSON.stringify({
          topic: "agent workflow",
          nodes: [
            {
              id: "topic:agent-workflow",
              kind: "topic",
              title: "agent workflow",
              summary: "Central topic",
              importance: 1,
              sourceItemIds: [],
              tags: []
            },
            {
              id: "hotspot:workflow",
              kind: "hotspot",
              title: "workflow",
              summary: "Core orchestration theme.",
              importance: 0.9,
              sourceItemIds: [],
              tags: ["workflow"]
            },
            {
              id: "source:github:agent",
              kind: "project",
              title: "acme/agent-workflow",
              summary: "Open source workflow runtime for agents.",
              importance: 0.8,
              sourceItemIds: ["github:agent"],
              tags: ["workflow"]
            }
          ],
          edges: [
            {
              id: "topic:agent-workflow->hotspot:workflow:mentions",
              from: "topic:agent-workflow",
              to: "hotspot:workflow",
              relation: "mentions",
              evidenceItemIds: []
            },
            {
              id: "hotspot:workflow->source:github:agent:implements",
              from: "hotspot:workflow",
              to: "source:github:agent",
              relation: "implements",
              evidenceItemIds: ["github:agent"]
            }
          ],
          caveats: ["model coverage is partial"]
        }),
        "```"
      ].join("\n")
    });

    const details = nodeDetailsFor(graph, "hotspot:workflow", sourceScan);

    expect(details?.node.title).toBe("workflow");
    expect(details?.connectedNodes.map((connection) => connection.node.id)).toContain("source:github:agent");
    expect(details?.sourceItems.map((item) => item.id)).toContain("github:agent");
    expect(details?.caveats).toContain("model coverage is partial");
    expect(details?.suggestedNextAction.length).toBeGreaterThan(0);
  });

  it("returns node details by id", () => {
    const graph = buildResearchGraph({
      topic: "agent workflow",
      sourceScan: null,
      parsedReport: { structured: false, sections: [] },
      finalOutput: ""
    });

    expect(nodeDetailsFor(graph, "topic:agent-workflow")?.node.title).toBe("agent workflow");
    expect(nodeDetailsFor(graph, "missing")).toBeNull();
  });
});
