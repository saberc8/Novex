import { describe, expect, it } from "vitest";
import { parseResearchReport } from "./research-report";

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
});
