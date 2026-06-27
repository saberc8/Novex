import { describe, expect, it, vi } from "vitest";
import {
  buildResearchTopicPlannerAgentRunCommand,
  buildResearchRadarAgentRunCommand,
  buildResearchRadarRepairAgentRunCommand,
  buildResearchRadarFallbackReport,
  buildFallbackResearchTopicPlan,
  configuredModelRouteOptions,
  parseResearchTopicPlanFromRun,
  createResearchRadarRepairRun,
  createResearchRadarRun
} from "./research";

describe("research radar agent command", () => {
  it("builds a topic planner command before source search", () => {
    const command = buildResearchTopicPlannerAgentRunCommand({
      topic: "量化因子",
      filters: ["papers", "projects", "datasets", "benchmarks"],
      ranking: "balanced",
      routeId: "runtime.llm",
      locale: "zh-CN"
    });

    expect(command.runtimeMode).toBe("model_loop");
    expect(command.modelRouteId).toBe("runtime.llm");
    expect(command.budget).toEqual({
      maxSteps: 3,
      maxToolCalls: 0,
      maxSeconds: 90,
      maxCostCents: 0
    });
    expect(command.workbenchContext).toEqual({
      mode: "agent",
      documentIds: [],
      fileIds: [],
      skillCodes: [],
      mcpToolCodes: [],
      webSearchEnabled: false,
      routeId: "runtime.llm"
    });
    expect(command.input).toContain("Research topic: 量化因子");
    expect(command.input).toContain("Do not browse or call tools");
    expect(command.input).toContain("research-topic-plan-json");
    expect(command.input).toContain("searchQueries");
    expect(command.input).toContain("relevanceKeywords");
    expect(command.input.length).toBeLessThanOrEqual(4000);
  });

  it("parses a fenced topic plan from the planner run output", () => {
    const plan = parseResearchTopicPlanFromRun({
      finalOutput: [
        "```research-topic-plan-json",
        JSON.stringify({
          topic: "量化因子",
          summary: "用价格、成交量和基本面构造可检验的 alpha 信号。",
          domains: ["金融工程", "机器学习"],
          learningGoals: ["理解因子定义", "掌握回测协议"],
          keyConcepts: ["alpha factor", "IC", "neutralization"],
          searchQueries: ["quant factor investing", "qlib factor research"],
          relevanceKeywords: ["factor", "alpha", "backtest", "IC"],
          sourcePriorities: ["papers", "projects", "datasets"]
        }),
        "```"
      ].join("\n")
    });

    expect(plan.topic).toBe("量化因子");
    expect(plan.searchQueries).toContain("quant factor investing");
    expect(plan.relevanceKeywords).toContain("backtest");
    expect(plan.sourcePriorities).toEqual(["papers", "projects", "datasets"]);
  });

  it("falls back to a generic topic plan when planner output is malformed", () => {
    const plan = parseResearchTopicPlanFromRun({
      finalOutput: "I cannot produce JSON."
    }, "量化因子", ["papers", "projects"]);

    expect(plan.topic).toBe("量化因子");
    expect(plan.searchQueries).toContain("量化因子");
    expect(plan.searchQueries).toContain("quant factor investing");
    expect(plan.searchQueries).toContain("qlib factor research");
    expect(plan.relevanceKeywords).toContain("量化因子");
    expect(plan.relevanceKeywords).toContain("alpha");
    expect(plan.relevanceKeywords).toContain("backtest");
    expect(plan.sourcePriorities).toEqual(["papers", "projects"]);
  });

  it("builds useful fallback queries when the topic planner is unavailable", () => {
    const plan = buildFallbackResearchTopicPlan("量化因子", ["papers", "projects", "datasets"]);

    expect(plan.summary).toContain("量化因子");
    expect(plan.domains).toEqual(expect.arrayContaining(["量化投资", "金融工程"]));
    expect(plan.searchQueries).toEqual(expect.arrayContaining([
      "量化因子",
      "quant factor investing",
      "alpha factor model",
      "qlib factor research",
      "factor investing dataset benchmark"
    ]));
    expect(plan.relevanceKeywords).toEqual(expect.arrayContaining([
      "量化因子",
      "factor",
      "alpha",
      "backtest",
      "IC"
    ]));
  });

  it("builds a model loop command with web search enabled", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "LLM agent memory",
      filters: ["papers", "projects", "benchmarks"],
      ranking: "recency",
      routeId: "runtime.llm.code_agent"
    });

    expect(command.runtimeMode).toBe("model_loop");
    expect(command.autoApprove).toBe(false);
    expect(command.modelRouteId).toBe("runtime.llm.code_agent");
    expect(command.budget).toEqual({
      maxSteps: 10,
      maxToolCalls: 6,
      maxSeconds: 180,
      maxCostCents: 0
    });
    expect(command.workbenchContext).toEqual({
      mode: "agent",
      documentIds: [],
      fileIds: [],
      skillCodes: [],
      mcpToolCodes: [],
      webSearchEnabled: true,
      routeId: "runtime.llm.code_agent"
    });
    expect(command.input).toContain("LLM agent memory");
    expect(command.input).toContain("Papers, Open source projects, Benchmarks");
    expect(command.input).toContain("Recency");
    expect(command.input).toContain("Use at most 3 web search calls total");
    expect(command.input).toContain("synthesize the report with caveats instead of searching again");
    expect(command.input).toContain("## 研究概览");
    expect(command.input).toContain("## 来源与限制");
  });

  it("uses configured route options from environment", () => {
    vi.stubEnv(
      "NEXT_PUBLIC_AGENT_MODEL_ROUTE_OPTIONS",
      "runtime.llm:Default Radar,runtime.llm.deep:Deep Research"
    );
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", "runtime.llm.deep");

    expect(configuredModelRouteOptions()).toEqual([
      { routeId: "runtime.llm.deep", label: "Deep Research" },
      { routeId: "runtime.llm", label: "Default Radar" }
    ]);
  });

  it("includes backend source evidence in the Agent prompt", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "agent workflow",
      filters: ["papers", "projects"],
      ranking: "balanced",
      routeId: "runtime.llm",
      sourceScan: {
        topic: "agent workflow",
        ranking: "balanced",
        status: "partial",
        warnings: ["leaderboards unavailable"],
        promptContext: "Research Radar Evidence\n[arxiv] Paper: Agent Workflow Planning",
        sources: [],
        items: []
      }
    });

    expect(command.input).toContain("Research Radar Evidence");
    expect(command.input).toContain("[arxiv] Paper: Agent Workflow Planning");
    expect(command.input).toContain("Use the provided backend source evidence first");
  });

  it("includes topic planner guidance in the Agent report prompt", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "量化因子",
      filters: ["papers", "projects", "datasets"],
      ranking: "balanced",
      routeId: "runtime.llm",
      topicPlan: {
        topic: "量化因子",
        summary: "用金融数据构造并验证 alpha 信号。",
        domains: ["金融工程", "机器学习"],
        learningGoals: ["理解因子定义", "掌握回测协议"],
        keyConcepts: ["alpha factor", "IC", "neutralization"],
        searchQueries: ["quant factor investing", "qlib factor research"],
        relevanceKeywords: ["factor", "alpha", "backtest", "IC"],
        sourcePriorities: ["papers", "projects", "datasets"]
      }
    });

    expect(command.input).toContain("Topic planner");
    expect(command.input).toContain("用金融数据构造并验证 alpha 信号");
    expect(command.input).toContain("Learning goals: 理解因子定义; 掌握回测协议");
    expect(command.input).toContain("Search queries: quant factor investing; qlib factor research");
    expect(command.input).toContain("Do not return raw tool_call JSON");
    expect(command.input.length).toBeLessThanOrEqual(4000);
  });

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
      command.input.indexOf("## 研究概览")
    );
    expect(command.input.length).toBeLessThanOrEqual(4000);
  });

  it("asks for a Chinese markdown report by default", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "agent workflow",
      filters: ["papers"],
      ranking: "balanced",
      routeId: "runtime.llm"
    });

    expect(command.input).toContain("请用中文撰写 markdown 报告");
    expect(command.input).toContain("## 研究概览");
    expect(command.input).toContain("## 来源与限制");
    expect(command.input).not.toContain("## Research Overview");
  });

  it("asks for an English markdown report when English is selected", () => {
    const command = buildResearchRadarAgentRunCommand({
      topic: "agent workflow",
      filters: ["papers"],
      ranking: "balanced",
      routeId: "runtime.llm",
      locale: "en-US"
    });

    expect(command.input).toContain("Write the markdown report in English");
    expect(command.input).toContain("## Research Overview");
    expect(command.input).toContain("## Sources And Caveats");
    expect(command.input).not.toContain("## 研究概览");
  });

  it("keeps Agent input within the backend character limit when source evidence is long", () => {
    const longEvidence = [
      "Research Radar Evidence",
      ...Array.from({ length: 80 }, (_, index) =>
        `[github] Project: agent-${index}\nSummary: ${"long source summary ".repeat(8)}`
      )
    ].join("\n");

    const command = buildResearchRadarAgentRunCommand({
      topic: "agent workflow",
      filters: ["papers", "projects", "datasets", "benchmarks"],
      ranking: "balanced",
      routeId: "runtime.llm",
      sourceScan: {
        topic: "agent workflow",
        ranking: "balanced",
        status: "partial",
        warnings: [],
        promptContext: longEvidence,
        sources: [],
        items: []
      }
    });

    expect(command.input.length).toBeLessThanOrEqual(4000);
    expect(command.input).toContain("Source evidence truncated to fit Agent input limit");
    expect(command.input).toContain("## 来源与限制");
  });

  it("builds a no-tool repair command for incomplete model reports", () => {
    const command = buildResearchRadarRepairAgentRunCommand({
      topic: "量化因子",
      filters: ["papers", "projects", "datasets", "benchmarks"],
      ranking: "balanced",
      routeId: "runtime.llm",
      locale: "zh-CN",
      previousOutput: [
        "还需要再搜索一次。",
        JSON.stringify({
          type: "tool_call",
          callId: "call-3",
          toolCode: "web.search"
        })
      ].join("\n"),
      sourceScan: {
        topic: "量化因子",
        ranking: "balanced",
        status: "partial",
        warnings: ["Papers With Code: not configured"],
        promptContext: "Research Radar Evidence\n[github] Project: microsoft/qlib",
        sources: [],
        items: []
      }
    });

    expect(command.runtimeMode).toBe("model_loop");
    expect(command.autoApprove).toBe(false);
    expect(command.budget).toEqual({
      maxSteps: 3,
      maxToolCalls: 1,
      maxSeconds: 90,
      maxCostCents: 0
    });
    expect(command.workbenchContext).toMatchObject({
      mode: "agent",
      webSearchEnabled: false,
      routeId: "runtime.llm"
    });
    expect(command.input).toContain("Repair invalid Research Radar report");
    expect(command.input).toContain("Do not call tools");
    expect(command.input).toContain("microsoft/qlib");
    expect(command.input).toContain("tool_call");
    expect(command.input).toContain("## 研究概览");
    expect(command.input).toContain("## 来源与限制");
    expect(command.input.length).toBeLessThanOrEqual(4000);
  });

  it("posts a research scan to the Agent run endpoint", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 77, traceId: "agent-77", status: "succeeded" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_API_BASE_URL", "http://localhost:62601");

    await expect(
      createResearchRadarRun({
        topic: "multimodal RAG",
        filters: ["papers", "datasets"],
        ranking: "importance",
        routeId: "runtime.llm"
      })
    ).resolves.toMatchObject({ runId: 77, traceId: "agent-77" });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/agents/runs",
      expect.objectContaining({
        method: "POST",
        body: expect.stringContaining('"webSearchEnabled":true')
      })
    );
  });

  it("posts a no-tool repair run to the Agent run endpoint", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 78, traceId: "agent-78", status: "succeeded" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_API_BASE_URL", "http://localhost:62601");

    await expect(
      createResearchRadarRepairRun({
        topic: "量化因子",
        filters: ["papers", "projects"],
        ranking: "balanced",
        routeId: "runtime.llm",
        previousOutput: "raw tool_call"
      })
    ).resolves.toMatchObject({ runId: 78, traceId: "agent-78" });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/agents/runs",
      expect.objectContaining({
        method: "POST",
        body: expect.stringContaining('"webSearchEnabled":false')
      })
    );
  });

  it("builds a structured fallback report from source evidence when model repair is unavailable", () => {
    const report = buildResearchRadarFallbackReport({
      topic: "量化因子",
      filters: ["papers", "projects", "datasets", "benchmarks"],
      ranking: "balanced",
      locale: "zh-CN",
      topicPlan: {
        topic: "量化因子",
        summary: "用金融数据构造并验证 alpha 信号。",
        domains: ["金融工程", "机器学习"],
        learningGoals: ["理解因子定义", "掌握回测协议"],
        keyConcepts: ["alpha factor", "IC", "backtest"],
        searchQueries: ["quant factor investing", "qlib factor research"],
        relevanceKeywords: ["factor", "alpha", "backtest"],
        sourcePriorities: ["papers", "projects", "datasets"]
      },
      sourceScan: {
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
    });

    expect(report).toContain("```research-graph-json");
    expect(report).toContain("microsoft/qlib");
    expect(report).toContain("Papers With Code-compatible endpoint is not configured");
    expect(report).toContain("## 研究概览");
    expect(report).toContain("## 活跃议题");
    expect(report).toContain("## 关键作者与机构");
    expect(report).toContain("## 代表性工作");
    expect(report).toContain("## 阅读路线");
    expect(report).toContain("## 研究切入点");
    expect(report).toContain("## 实验方案");
    expect(report).toContain("## 来源与限制");
  });
});
