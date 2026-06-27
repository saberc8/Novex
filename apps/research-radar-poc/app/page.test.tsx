import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Page from "./page";
import { EvidenceRail, SourceResults } from "@/app-client";
import { researchRadarCopy } from "@/lib/i18n";

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

  it("localizes model selector accessibility labels with the active locale", () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, json: async () => ({ code: "200", data: {} }) })));

    render(<Page />);

    expect(screen.getByRole("button", { name: "选择模型 runtime.llm" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "选择模型 runtime.llm" }));
    expect(screen.getByRole("listbox", { name: "模型列表" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "选择模型 runtime.llm" }));

    fireEvent.click(screen.getByRole("button", { name: "English" }));

    expect(screen.getByRole("button", { name: "Choose model runtime.llm" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Choose model runtime.llm" }));
    expect(screen.getByRole("listbox", { name: "Model list" })).toBeTruthy();
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

    const runCall = (fetchMock.mock.calls as unknown as Array<[string, RequestInit | undefined]>).find(([url, init]) =>
      String(url).includes("/ai/agents/runs") &&
      !String(url).includes("/events") &&
      String((init as RequestInit | undefined)?.body).includes('"webSearchEnabled":true')
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

    const runCall = (fetchMock.mock.calls as unknown as Array<[string, RequestInit | undefined]>).find(([url, init]) =>
      String(url).includes("/ai/agents/runs") &&
      !String(url).includes("/events") &&
      String((init as RequestInit | undefined)?.body).includes('"webSearchEnabled":true')
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

    expect(await screen.findByText("研究图谱")).toBeTruthy();
    expect(await screen.findByRole("button", { name: /acme\/agent/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: "证据抽屉" })).toBeTruthy();
    expect(await screen.findByText("leaderboards unavailable")).toBeTruthy();
    const reportRunIndex = (fetchMock.mock.calls as unknown as Array<[string, RequestInit | undefined]>).findIndex(([url, init]) =>
      String(url).includes("/ai/agents/runs") &&
      !String(url).includes("/events") &&
      String((init as RequestInit | undefined)?.body).includes('"webSearchEnabled":true')
    );
    expect(calls.findIndex((url) => url.includes("/ai/research-radar/scans"))).toBeLessThan(reportRunIndex);

    const runCall = fetchMock.mock.calls[reportRunIndex] as unknown as [string, RequestInit];
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
    expect((fetchMock.mock.calls as unknown as Array<[string, RequestInit | undefined]>).some(([url, init]) =>
      String(url).includes("/ai/agents/runs") &&
      String((init as RequestInit | undefined)?.body).includes('"webSearchEnabled":true')
    )).toBe(false);
  });

  it("repairs raw tool-call final output with a no-tool report run", async () => {
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
      const href = String(url);
      const body = String(init?.body ?? "");
      if (href.includes("/ai/research-radar/scans")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              topic: "量化因子",
              ranking: "balanced",
              status: "succeeded",
              warnings: [],
              promptContext: "Research Radar Evidence\n[github] Project: microsoft/qlib",
              sources: [],
              items: []
            }
          })
        };
      }
      if (
        href.includes("/ai/agents/runs") &&
        body.includes('"webSearchEnabled":false') &&
        body.includes("Repair invalid Research Radar report")
      ) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              runId: 92,
              traceId: "agent-92",
              status: "succeeded",
              finalOutput: [
                "## 研究概览",
                "量化因子研究应从因子假设、数据清洗、横截面检验和组合回测一起理解。",
                "## 活跃议题",
                "- 因子拥挤、非平稳性和交易成本鲁棒性",
                "## 关键作者与机构",
                "金融工程团队、开源量化社区和机器学习研究者。",
                "## 代表性工作",
                "microsoft/qlib 可作为工程化实验入口。",
                "## 阅读路线",
                "先读因子定义与 IC，再读回测和组合构建。",
                "## 研究切入点",
                "比较公开因子在不同市场 regime 下的稳定性。",
                "## 实验方案",
                "复现一个 alpha 因子，做中性化、分组收益和换手率分析。",
                "## 来源与限制",
                "Papers With Code 与榜单覆盖受限，应标记为缺口。"
              ].join("\n")
            }
          })
        };
      }
      if (href.includes("/ai/agents/runs") && body.includes('"webSearchEnabled":false')) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              runId: 90,
              traceId: "agent-90",
              status: "succeeded",
              finalOutput: [
                "```research-topic-plan-json",
                JSON.stringify({
                  topic: "量化因子",
                  summary: "研究 alpha 因子的定义、验证和回测。",
                  domains: ["金融工程"],
                  learningGoals: ["理解 IC"],
                  keyConcepts: ["IC", "alpha factor"],
                  searchQueries: ["quant factor investing"],
                  relevanceKeywords: ["factor", "alpha", "IC"],
                  sourcePriorities: ["papers", "projects"]
                }),
                "```"
              ].join("\n")
            }
          })
        };
      }
      if (href.includes("/ai/agents/runs") && body.includes('"webSearchEnabled":true')) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              runId: 91,
              traceId: "agent-91",
              status: "succeeded",
              finalOutput: [
                "还需要再搜索一次。",
                "```json",
                JSON.stringify({
                  type: "tool_call",
                  callId: "call-3",
                  toolCode: "web.search",
                  arguments: { query: "量化因子 benchmark" }
                }),
                "```"
              ].join("\n")
            }
          })
        };
      }
      if (href.includes("/events")) {
        return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
      }
      return { ok: true, json: async () => ({ code: "200", data: {} }) };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.change(screen.getByLabelText("研究主题"), {
      target: { value: "量化因子" }
    });
    fireEvent.click(screen.getByRole("button", { name: "启动雷达扫描" }));

    expect(await screen.findByText("量化因子研究应从因子假设、数据清洗、横截面检验和组合回测一起理解。")).toBeTruthy();
    expect(await screen.findByText("运行 #92")).toBeTruthy();
    expect(screen.queryByRole("alert")?.textContent ?? "").not.toContain("模型分析未完成");

    const repairCall = (fetchMock.mock.calls as unknown as Array<[string, RequestInit | undefined]>).find(([url, init]) =>
      String(url).includes("/ai/agents/runs") &&
      String((init as RequestInit | undefined)?.body).includes("Repair invalid Research Radar report")
    ) as unknown as [string, RequestInit];
    expect(String(repairCall[1].body)).toContain('"webSearchEnabled":false');
    expect(String(repairCall[1].body)).toContain("tool_call");
  });

  it("shows a deterministic fallback report when the repair run cannot be created", async () => {
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
      const href = String(url);
      const body = String(init?.body ?? "");
      if (href.includes("/ai/research-radar/scans")) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              topic: "量化因子",
              ranking: "balanced",
              status: "partial",
              warnings: [
                "Papers With Code: Papers With Code-compatible endpoint is not configured",
                "Leaderboards: leaderboard endpoints are not configured"
              ],
              promptContext: "Research Radar Evidence\n[github] Project: microsoft/qlib",
              sources: [
                {
                  source: "github",
                  status: "succeeded",
                  warning: null,
                  items: [
                    {
                      id: "github:microsoft/qlib",
                      source: "github",
                      kind: "project",
                      title: "microsoft/qlib",
                      url: "https://github.com/microsoft/qlib",
                      summary: "AI-oriented quantitative investment platform.",
                      authors: [],
                      organization: "Microsoft",
                      publishedAt: null,
                      updatedAt: "2026-06-01T00:00:00Z",
                      metrics: [{ label: "stars", value: 18000 }],
                      tags: ["finance", "factor"],
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
      if (
        href.includes("/ai/agents/runs") &&
        body.includes('"webSearchEnabled":false') &&
        body.includes("Repair invalid Research Radar report")
      ) {
        return {
          ok: false,
          json: async () => ({
            code: "400",
            msg: "工具调用预算不足"
          })
        };
      }
      if (href.includes("/ai/agents/runs") && body.includes('"webSearchEnabled":false')) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              runId: 90,
              traceId: "agent-90",
              status: "succeeded",
              finalOutput: "not json"
            }
          })
        };
      }
      if (href.includes("/ai/agents/runs") && body.includes('"webSearchEnabled":true')) {
        return {
          ok: true,
          json: async () => ({
            code: "200",
            data: {
              runId: 91,
              traceId: "agent-91",
              status: "succeeded",
              finalOutput: "```research-graph-json\n{\"topic\":\"量化因子\",\"nodes\":["
            }
          })
        };
      }
      if (href.includes("/events")) {
        return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
      }
      return { ok: true, json: async () => ({ code: "200", data: {} }) };
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Page />);

    fireEvent.change(screen.getByLabelText("研究主题"), {
      target: { value: "量化因子" }
    });
    fireEvent.click(screen.getByRole("button", { name: "启动雷达扫描" }));

    expect(await screen.findByText(/基于已收集证据生成的兜底分析/)).toBeTruthy();
    expect(await screen.findByText("microsoft/qlib")).toBeTruthy();
    expect(screen.queryByRole("alert")?.textContent ?? "").not.toContain("模型分析未完成");
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

    expect(await screen.findByText("节点详情")).toBeTruthy();
    expect(await screen.findByText("Execution consistency across long-horizon coding tasks.")).toBeTruthy();
    expect(await screen.findByText("关联证据")).toBeTruthy();
    expect((await screen.findByRole("link", { name: /acme\/agent/ })).getAttribute("href")).toBe(
      "https://github.com/acme/agent"
    );
    expect((await screen.findAllByText("Source coverage is partial.")).length).toBeGreaterThan(0);
    expect(await screen.findByText("来源链接")).toBeTruthy();
    expect(await screen.findByText("建议下一步")).toBeTruthy();
  });

  it("localizes inspector node-kind labels in Chinese", () => {
    const copy = researchRadarCopy("zh-CN");

    render(
      <EvidenceRail
        activeScan={null}
        copy={copy.evidence}
        eventEvidence={[]}
        inspectorCopy={copy.inspector}
        mapCopy={copy.map}
        modelDeltaSummary={null}
        researchGraph={null}
        selectedGraphNode={{
          node: {
            id: "question:planning",
            kind: "open_question",
            title: "Planning reliability",
            summary: "How should long-horizon plans be stabilized?",
            importance: 0.74,
            sourceItemIds: [],
            tags: []
          },
          connectedNodes: [
            {
              node: {
                id: "project:agent-runtime",
                kind: "project",
                title: "agent-runtime",
                summary: "Workflow runtime",
                importance: 0.64,
                sourceItemIds: [],
                tags: []
              },
              relation: "implements",
              direction: "incoming",
              evidenceItemIds: []
            }
          ],
          sourceItemIds: [],
          sourceItems: [],
          caveats: [],
          suggestedNextAction: "Compare planning loops."
        }}
        statusCopy={copy.status}
      />
    );

    expect(screen.getByText("类型 开放问题")).toBeTruthy();
    expect(screen.getByText("证据 1")).toBeTruthy();
    expect(screen.getByText("项目")).toBeTruthy();
    expect(screen.getByText("实现")).toBeTruthy();
    expect(screen.queryByText("implements")).toBeNull();
  });

  it("localizes inspector fallback copy in Chinese", () => {
    const copy = researchRadarCopy("zh-CN");

    render(
      <EvidenceRail
        activeScan={null}
        copy={copy.evidence}
        eventEvidence={[]}
        inspectorCopy={copy.inspector}
        mapCopy={copy.map}
        modelDeltaSummary={null}
        researchGraph={null}
        selectedGraphNode={{
          node: {
            id: "question:planning",
            kind: "open_question",
            title: "Planning reliability",
            summary: "",
            importance: 0.74,
            sourceItemIds: [],
            tags: []
          },
          connectedNodes: [],
          sourceItemIds: [],
          sourceItems: [],
          caveats: [],
          suggestedNextAction: "Compare planning loops."
        }}
        statusCopy={copy.status}
      />
    );

    expect(screen.getByText("暂无节点摘要。")).toBeTruthy();
    expect(screen.getByText("暂无关联证据。")).toBeTruthy();
    expect(screen.getByText("暂无来源链接。")).toBeTruthy();
  });

  it("localizes source result status badges in Chinese", () => {
    const copy = researchRadarCopy("zh-CN");

    render(
      <SourceResults
        copy={copy.drawer}
        sources={[
          {
            source: "github",
            status: "succeeded",
            warning: null,
            items: []
          },
          {
            source: "leaderboards",
            status: "degraded",
            warning: "coverage limited",
            items: []
          },
          {
            source: "arxiv",
            status: "failed",
            warning: "request failed",
            items: []
          }
        ]}
        statusCopy={copy.status}
      />
    );

    expect(screen.getByText("就绪")).toBeTruthy();
    expect(screen.getByText("受限")).toBeTruthy();
    expect(screen.getByText("失败")).toBeTruthy();
  });

  it("localizes source result item-kind chips in Chinese", () => {
    const copy = researchRadarCopy("zh-CN");

    render(
      <SourceResults
        copy={copy.drawer}
        sources={[
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
                organization: "Acme",
                publishedAt: null,
                updatedAt: "2026-06-01T00:00:00Z",
                metrics: [],
                tags: ["workflow"],
                metadata: {}
              },
              {
                id: "arxiv:1234.5678",
                source: "github",
                kind: "paper",
                title: "Planning for Agents",
                url: "https://arxiv.org/abs/1234.5678",
                summary: "Paper summary",
                authors: ["Ada Lovelace"],
                organization: null,
                publishedAt: "2026-05-01T00:00:00Z",
                updatedAt: null,
                metrics: [],
                tags: ["planning"],
                metadata: {}
              }
            ]
          }
        ]}
        statusCopy={copy.status}
      />
    );

    expect(screen.getByText("项目")).toBeTruthy();
    expect(screen.getByText("论文")).toBeTruthy();
    expect(screen.queryByText("project")).toBeNull();
    expect(screen.queryByText("paper")).toBeNull();
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

    expect(await screen.findByText("研究图谱")).toBeTruthy();
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

    expect(await screen.findByText("研究图谱")).toBeTruthy();
    expect(await screen.findByText("暂无可用图谱节点")).toBeTruthy();
    expect(await screen.findByText("leaderboards unavailable")).toBeTruthy();
  });

  it("switches graph surfaces to English after locale selection", async () => {
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

    fireEvent.click(screen.getByRole("button", { name: "English" }));
    fireEvent.change(screen.getByLabelText("Research topic"), {
      target: { value: "AI coding agents" }
    });
    fireEvent.click(screen.getByRole("button", { name: "Start radar scan" }));

    expect(await screen.findByText("Research Map")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Evidence Drawer" })).toBeTruthy();
  });
});
