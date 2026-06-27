import { describe, expect, it } from "vitest";
import { parseResearchReport, evaluateResearchReportQuality } from "./research-report";

describe("parseResearchReport", () => {
  it("parses expected markdown headings into ordered sections", () => {
    const report = parseResearchReport(`
## Research Overview
Agent memory connects episodic traces with durable user preferences.

## Active Topics
- Long-context memory
- Retrieval-augmented memory

## Key Authors And Institutions
Stanford, Berkeley, OpenAI, and community agent frameworks.

## Representative Work
MemGPT and Reflexion are useful starting points.

## Reading Route
Start with surveys, then read system papers.

## Research Openings
Evaluate memory under distribution shift.

## Experiment Plans
Build an ablation around recall precision.

## Sources And Caveats
Search coverage may miss workshop papers.
`);

    expect(report.structured).toBe(true);
    expect(report.sections.map((section) => section.title)).toEqual([
      "Research Overview",
      "Active Topics",
      "Key Authors And Institutions",
      "Representative Work",
      "Reading Route",
      "Research Openings",
      "Experiment Plans",
      "Sources And Caveats"
    ]);
    expect(report.sections[0].content).toContain("episodic traces");
    expect(report.sections[7].content).toContain("workshop papers");
  });

  it("falls back to a raw report when expected headings are missing", () => {
    const report = parseResearchReport("A loose answer without the agreed heading contract.");

    expect(report).toEqual({
      structured: false,
      sections: [
        {
          id: "raw",
          title: "Research Report",
          content: "A loose answer without the agreed heading contract."
        }
      ]
    });
  });

  it("marks raw tool-call JSON as unfinished analysis", () => {
    const quality = evaluateResearchReportQuality(`
The next useful step is another search.

\`\`\`json
{"type":"tool_call","callId":"call-3","toolCode":"web.search","arguments":{"query":"量化因子 benchmark"}}
\`\`\`
`);

    expect(quality).toEqual({
      ok: false,
      reason: "unfinished_tool_call"
    });
  });

  it("marks unstructured markdown as incomplete", () => {
    const quality = evaluateResearchReportQuality("A loose answer without the agreed heading contract.");

    expect(quality).toEqual({
      ok: false,
      reason: "missing_sections"
    });
  });

  it("parses Chinese headings into structured sections with stable ids", () => {
    const report = parseResearchReport(`
## 研究概览
中文总览。

## 活跃议题
- 代理工作流

## 关键作者与机构
清华大学与 OpenAI。

## 代表性工作
Representative paper

## 阅读路线
先读综述。

## 研究切入点
关注评测缺口。

## 实验方案
做消融实验。

## 来源与限制
来源覆盖有限。
`);

    expect(report.structured).toBe(true);
    expect(report.sections.map((section) => section.id)).toEqual([
      "research-overview",
      "active-topics",
      "key-authors-and-institutions",
      "representative-work",
      "reading-route",
      "research-openings",
      "experiment-plans",
      "sources-and-caveats"
    ]);
    expect(report.sections.map((section) => section.title)).toEqual([
      "研究概览",
      "活跃议题",
      "关键作者与机构",
      "代表性工作",
      "阅读路线",
      "研究切入点",
      "实验方案",
      "来源与限制"
    ]);
    expect(report.sections[0].content).toContain("中文总览");
    expect(report.sections[7].content).toContain("来源覆盖有限");
    expect(evaluateResearchReportQuality(report.sections.map((section) => `## ${section.title}\n${section.content}`).join("\n\n"))).toEqual({
      ok: true
    });
  });
});
