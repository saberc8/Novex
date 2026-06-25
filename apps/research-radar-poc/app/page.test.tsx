import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Page from "./page";

describe("Research Radar POC page", () => {
  afterEach(() => {
    window.localStorage.clear();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it("renders the workbench in Chinese by default", () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        json: async () => ({ code: "200", data: { list: [], total: 0 } })
      }))
    );

    render(<Page />);

    expect(screen.getByRole("heading", { name: "Research Radar" })).toBeTruthy();
    expect(screen.getByLabelText("研究主题")).toBeTruthy();
    expect(screen.getByText("论文")).toBeTruthy();
    expect(screen.getByText("开源项目")).toBeTruthy();
    expect(screen.getByText("数据集")).toBeTruthy();
    expect(screen.getByText("基准")).toBeTruthy();
    expect(screen.getByText("新闻")).toBeTruthy();
    expect(screen.getByText("社区")).toBeTruthy();
    expect(screen.getByRole("button", { name: "均衡" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "启动雷达扫描" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "中文" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "English" })).toBeTruthy();
  });

  it("switches visible workbench copy to English", () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, json: async () => ({ code: "200", data: {} }) })));

    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "English" }));

    expect(screen.getByLabelText("Research topic")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Balanced" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start radar scan" })).toBeTruthy();
    expect(window.localStorage.getItem("novex.researchRadar.locale")).toBe("en-US");
  });

  it("restores saved English locale and ignores invalid saved locale", () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, json: async () => ({ code: "200", data: {} }) })));
    window.localStorage.setItem("novex.researchRadar.locale", "en-US");

    const { unmount } = render(<Page />);
    expect(screen.getByLabelText("Research topic")).toBeTruthy();
    unmount();

    window.localStorage.setItem("novex.researchRadar.locale", "bad");
    render(<Page />);
    expect(screen.getByLabelText("研究主题")).toBeTruthy();
  });

  it("re-renders app-generated run errors after switching locale", async () => {
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
              status: "partial",
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
                      summary: "Agent workflows",
                      authors: [],
                      organization: null,
                      publishedAt: null,
                      updatedAt: "2026-06-01T00:00:00Z",
                      metrics: [{ label: "stars", value: 1200 }],
                      tags: ["agents"],
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
          ok: false,
          json: async () => ({
            code: "500",
            msg: "arbitrary backend failure"
          })
        };
      }
      return {
        ok: true,
        json: async () => ({ code: "200", data: {} })
      };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.change(screen.getByLabelText("研究主题"), {
      target: { value: "AI coding agents" }
    });
    fireEvent.click(screen.getByRole("button", { name: "启动雷达扫描" }));

    expect((await screen.findByRole("alert")).textContent).toContain("模型分析暂不可用");

    fireEvent.click(screen.getByRole("button", { name: "English" }));

    expect((await screen.findByRole("alert")).textContent).toContain("model analysis unavailable");
  });

  it("localizes the active run chip and pending section fallback in Chinese", async () => {
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
              promptContext: "Research Radar Evidence\n[arxiv] Paper: AI coding agents",
              sources: [],
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
              runId: 91,
              traceId: "agent-91",
              status: "succeeded",
              finalOutput: ""
            }
          })
        };
      }
      if (href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              list: [],
              total: 0
            }
          })
        };
      }
      return {
        ok: true,
        json: async () => ({ code: "200", data: {} })
      };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.change(screen.getByLabelText("研究主题"), {
      target: { value: "AI coding agents" }
    });
    fireEvent.click(screen.getByRole("button", { name: "启动雷达扫描" }));

    expect(await screen.findByText("运行 #91")).toBeTruthy();
    expect(await screen.findByText("等待模型输出")).toBeTruthy();
  });

  it("forwards the English report-language payload when English is selected", async () => {
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
              promptContext: "Research Radar Evidence\n[arxiv] Paper: AI coding agents",
              sources: [],
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
              runId: 91,
              traceId: "agent-91",
              status: "succeeded",
              finalOutput: "## Research Overview\nReport"
            }
          })
        };
      }
      if (href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              list: [],
              total: 0
            }
          })
        };
      }
      return {
        ok: true,
        json: async () => ({ code: "200", data: {} })
      };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "English" }));
    fireEvent.change(screen.getByLabelText("Research topic"), {
      target: { value: "AI coding agents" }
    });
    fireEvent.click(screen.getByRole("button", { name: "Start radar scan" }));

    await screen.findByText("Research Report");

    const runCall = fetchMock.mock.calls.find(([url]) =>
      String(url).includes("/ai/agents/runs") && !String(url).includes("/events")
    ) as unknown as [string, RequestInit];
    expect(String(runCall[1].body)).toContain("Write the markdown report in English");
  });

  it("localizes the generic fallback scan error in English", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        const href = String(url);
        if (href.includes("/ai/research-radar/scans")) {
          throw "network down";
        }
        return {
          ok: true,
          json: async () => ({ code: "200", data: {} })
        };
      })
    );

    render(<Page />);

    fireEvent.click(screen.getByRole("button", { name: "English" }));
    fireEvent.change(screen.getByLabelText("Research topic"), {
      target: { value: "AI coding agents" }
    });
    fireEvent.click(screen.getByRole("button", { name: "Start radar scan" }));

    expect((await screen.findByRole("alert")).textContent).toContain("Radar scan failed");
  });

  it("submits a topic and renders structured research output with evidence", async () => {
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
              promptContext: "Research Radar Evidence\n[arxiv] Paper: AI coding agents",
              sources: [],
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
              runId: 91,
              traceId: "agent-91",
              status: "succeeded",
              finalOutput: [
                "## Research Overview",
                "AI coding agents are moving from task completion toward durable engineering workflows.",
                "## Active Topics",
                "- Repository-scale context",
                "## Key Authors And Institutions",
                "Open source agent frameworks and industrial labs.",
                "## Representative Work",
                "SWE-bench and coding agent papers.",
                "## Reading Route",
                "Start with SWE-bench.",
                "## Research Openings",
                "Measure planning reliability.",
                "## Experiment Plans",
                "Run ablations across repository size.",
                "## Sources And Caveats",
                "Search coverage may be incomplete."
              ].join("\n")
            }
          })
        };
      }
      if (href.includes("/events")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              list: [
                {
                  id: 1,
                  runId: 91,
                  eventType: "thought",
                  sequenceNo: 1,
                  status: "running",
                  payload: { item: { type: "model_delta", deltaIndex: 0, content: "Live radar" } },
                  createTime: ""
                },
                {
                  id: 2,
                  runId: 91,
                  eventType: "thought",
                  sequenceNo: 2,
                  status: "succeeded",
                  payload: {
                    item: {
                      type: "tool_observation",
                      toolCode: "web.search",
                      output: { dryRun: true, status: "dry_run" }
                    }
                  },
                  createTime: ""
                }
              ],
              total: 2
            }
          })
        };
      }
      return {
        ok: true,
        json: async () => ({ code: "200", data: {} })
      };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.change(screen.getByLabelText("研究主题"), {
      target: { value: "AI coding agents" }
    });
    fireEvent.click(screen.getByRole("button", { name: "启动雷达扫描" }));

    expect(await screen.findByText("AI coding agents are moving from task completion toward durable engineering workflows.")).toBeTruthy();
    expect(await screen.findByText("运行 #91")).toBeTruthy();
    expect(await screen.findByText("Live radar")).toBeTruthy();
    expect(await screen.findByText("dry-run: web.search returned no live provider result")).toBeTruthy();

    const runCall = fetchMock.mock.calls.find(([url]) =>
      String(url).includes("/ai/agents/runs") && !String(url).includes("/events")
    ) as unknown as [string, RequestInit];
    expect(String(runCall[1].body)).toContain('"webSearchEnabled":true');
  });

  it("runs backend source scan before creating the Agent run", async () => {
    const calls: string[] = [];
    const fetchMock = vi.fn(async (url: string) => {
      const href = String(url);
      calls.push(href);
      if (href.includes("/ai/research-radar/scans")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              topic: "AI coding agents",
              ranking: "balanced",
              status: "partial",
              warnings: ["leaderboards unavailable"],
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
                      summary: "Agent workflows",
                      authors: [],
                      organization: null,
                      publishedAt: null,
                      updatedAt: "2026-06-01T00:00:00Z",
                      metrics: [{ label: "stars", value: 1200 }],
                      tags: ["agents"],
                      metadata: {}
                    }
                  ]
                },
                {
                  source: "leaderboards",
                  status: "failed",
                  warning: "leaderboards unavailable",
                  items: []
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
              runId: 91,
              traceId: "agent-91",
              status: "succeeded",
              finalOutput: "## Research Overview\nReport"
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

    expect(await screen.findByText("Research Map")).toBeTruthy();
    expect(await screen.findByRole("button", { name: /acme\/agent/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Evidence Drawer" })).toBeTruthy();
    expect(await screen.findByText("leaderboards unavailable")).toBeTruthy();
    expect(calls.findIndex((url) => url.includes("/ai/research-radar/scans"))).toBeLessThan(
      calls.findIndex((url) => url.includes("/ai/agents/runs"))
    );

    const runCall = fetchMock.mock.calls.find(([url]) =>
      String(url).includes("/ai/agents/runs") && !String(url).includes("/events")
    ) as unknown as [string, RequestInit];
    expect(String(runCall[1].body)).toContain("Research Radar Evidence");
  });

  it("does not create an Agent run when all source scans fail", async () => {
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
              status: "failed",
              warnings: ["all sources failed"],
              promptContext: "",
              sources: [],
              items: []
            }
          })
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

    expect(await screen.findByText("all sources failed")).toBeTruthy();
    expect(fetchMock.mock.calls.some(([url]) => String(url).includes("/ai/agents/runs"))).toBe(false);
  });

  it("updates the right rail with connected evidence, source links, caveats, and next action when a graph node is selected", async () => {
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
                      id: "hotspot:workflow-reliability",
                      kind: "hotspot",
                      title: "Workflow reliability",
                      summary: "Execution consistency across long-horizon coding tasks.",
                      importance: 0.88,
                      sourceItemIds: [],
                      tags: ["workflow", "reliability"]
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
                      id: "topic:ai-coding-agents->hotspot:workflow-reliability:mentions",
                      from: "topic:ai-coding-agents",
                      to: "hotspot:workflow-reliability",
                      relation: "mentions",
                      evidenceItemIds: []
                    },
                    {
                      id: "hotspot:workflow-reliability->source:github:acme/agent:implements",
                      from: "hotspot:workflow-reliability",
                      to: "source:github:acme/agent",
                      relation: "implements",
                      evidenceItemIds: ["github:acme/agent"]
                    }
                  ],
                  caveats: ["Source coverage is partial."]
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
    fireEvent.click(await screen.findByRole("button", { name: /Workflow reliability/ }));

    expect(await screen.findByText("Node Inspector")).toBeTruthy();
    expect(await screen.findByText("Execution consistency across long-horizon coding tasks.")).toBeTruthy();
    expect(await screen.findByText("Connected evidence")).toBeTruthy();
    expect((await screen.findByRole("link", { name: /acme\/agent/ })).getAttribute("href")).toBe(
      "https://github.com/acme/agent"
    );
    expect((await screen.findAllByText("Source coverage is partial.")).length).toBeGreaterThan(0);
    expect(await screen.findByText("Suggested next action")).toBeTruthy();
  });

  it("keeps the source-derived graph and shows a model degradation fallback when the Agent run fails", async () => {
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
              status: "partial",
              warnings: ["leaderboards unavailable"],
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
                },
                {
                  source: "leaderboards",
                  status: "failed",
                  warning: "leaderboards unavailable",
                  items: []
                }
              ],
              items: []
            }
          })
        };
      }
      if (href.includes("/ai/agents/runs") && !href.includes("/events")) {
        return {
          ok: false,
          json: async () => ({
            code: "500",
            msg: "arbitrary backend failure"
          })
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

    expect(await screen.findByText("Research Map")).toBeTruthy();
    expect(await screen.findByRole("button", { name: /acme\/agent/ })).toBeTruthy();
    expect((await screen.findByRole("alert")).textContent).toContain("模型分析暂不可用");
  });

  it("shows a clear empty graph state with source warnings when the model graph has no usable nodes", async () => {
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
              status: "partial",
              warnings: ["leaderboards unavailable"],
              promptContext: "Research Radar Evidence\nNo usable source items",
              sources: [
                {
                  source: "leaderboards",
                  status: "failed",
                  warning: "leaderboards unavailable",
                  items: []
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
              runId: 93,
              traceId: "agent-93",
              status: "succeeded",
              finalOutput: [
                "```research-graph-json",
                JSON.stringify({
                  topic: "AI coding agents",
                  nodes: [],
                  edges: [],
                  caveats: []
                }),
                "```",
                "## Research Overview",
                "Report"
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

    expect(await screen.findByText("Research Map")).toBeTruthy();
    expect(await screen.findByText("No usable graph nodes")).toBeTruthy();
    expect(await screen.findByText("leaderboards unavailable")).toBeTruthy();
  });
});
