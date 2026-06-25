import type { ParsedResearchReport } from "@/types/research";

const EXPECTED_HEADINGS = [
  "Research Overview",
  "Active Topics",
  "Key Authors And Institutions",
  "Representative Work",
  "Reading Route",
  "Research Openings",
  "Experiment Plans",
  "Sources And Caveats"
];

export function parseResearchReport(markdown: string): ParsedResearchReport {
  const trimmed = markdown.trim();
  const sections = extractHeadingSections(trimmed);
  const knownSections = EXPECTED_HEADINGS.flatMap((heading) =>
    sections.get(heading)
      ? [
          {
            id: slugifyHeading(heading),
            title: heading,
            content: sections.get(heading) ?? ""
          }
        ]
      : []
  );

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

function slugifyHeading(heading: string) {
  return heading.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
