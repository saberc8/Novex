import type { ParsedResearchReport } from "@/types/research";

const RESEARCH_REPORT_SECTIONS = [
  {
    id: "research-overview",
    headings: ["Research Overview", "研究概览"]
  },
  {
    id: "active-topics",
    headings: ["Active Topics", "活跃议题"]
  },
  {
    id: "key-authors-and-institutions",
    headings: ["Key Authors And Institutions", "关键作者与机构"]
  },
  {
    id: "representative-work",
    headings: ["Representative Work", "代表性工作"]
  },
  {
    id: "reading-route",
    headings: ["Reading Route", "阅读路线"]
  },
  {
    id: "research-openings",
    headings: ["Research Openings", "研究切入点"]
  },
  {
    id: "experiment-plans",
    headings: ["Experiment Plans", "实验方案"]
  },
  {
    id: "sources-and-caveats",
    headings: ["Sources And Caveats", "来源与限制"]
  }
] as const;

export function parseResearchReport(markdown: string): ParsedResearchReport {
  const trimmed = markdown.trim();
  const sections = extractHeadingSections(trimmed);
  const knownSections = RESEARCH_REPORT_SECTIONS.flatMap((section) => {
    const matchedHeading = section.headings.find((heading) => sections.has(heading));
    return matchedHeading
      ? [
          {
            id: section.id,
            title: matchedHeading,
            content: sections.get(matchedHeading) ?? ""
          }
        ]
      : [];
  });

  if (knownSections.length >= 4) {
    return {
      structured: true,
      sections: knownSections
    };
  }

  return {
    structured: false,
    sections: [
      {
        id: "raw",
        title: "Research Report",
        content: trimmed
      }
    ]
  };
}

export type ResearchReportQuality =
  | {
      ok: true;
    }
  | {
      ok: false;
      reason: "unfinished_tool_call" | "missing_sections";
    };

export function evaluateResearchReportQuality(
  markdown: string,
  parsedReport: ParsedResearchReport = parseResearchReport(markdown)
): ResearchReportQuality {
  if (containsRawToolCall(markdown)) {
    return {
      ok: false,
      reason: "unfinished_tool_call"
    };
  }

  if (!parsedReport.structured) {
    return {
      ok: false,
      reason: "missing_sections"
    };
  }

  return {
    ok: true
  };
}

function containsRawToolCall(markdown: string) {
  return /"type"\s*:\s*"tool_calls?"[\s\S]{0,800}"(?:callId|toolCode|calls)"/i.test(markdown);
}

function extractHeadingSections(markdown: string) {
  const result = new Map<string, string>();
  const headingPattern = /^##\s+(.+)$/gm;
  const matches = Array.from(markdown.matchAll(headingPattern));

  matches.forEach((match, index) => {
    const title = match[1]?.trim();
    if (!title) {
      return;
    }
    const contentStart = (match.index ?? 0) + match[0].length;
    const contentEnd = index + 1 < matches.length ? matches[index + 1].index ?? markdown.length : markdown.length;
    result.set(title, markdown.slice(contentStart, contentEnd).trim());
  });

  return result;
}
