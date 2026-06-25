import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Page from "./page";

describe("Research Radar POC page", () => {
  afterEach(() => {
    window.localStorage.clear();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it("renders the research radar workbench", () => {
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
    expect(screen.getByText("Papers")).toBeTruthy();
    expect(screen.getByText("Projects")).toBeTruthy();
    expect(screen.getByText("Datasets")).toBeTruthy();
    expect(screen.getByText("Benchmarks")).toBeTruthy();
    expect(screen.getByText("News")).toBeTruthy();
    expect(screen.getByText("Community")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Balanced" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "启动雷达扫描" })).toBeTruthy();
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
    expect(await screen.findByText("Run #91")).toBeTruthy();
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

    expect(await screen.findByText("Source Results")).toBeTruthy();
    expect(await screen.findByText("acme/agent")).toBeTruthy();
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
});
