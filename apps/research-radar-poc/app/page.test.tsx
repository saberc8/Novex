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
});
