import { apiRequest } from "@/lib/api";
import type {
  ResearchFilter,
  ResearchSource,
  ResearchSourceScanInput,
  ResearchSourceScanResp
} from "@/types/research";

const DEFAULT_LIMIT_PER_SOURCE = 5;

export function createResearchRadarSourceScan(input: ResearchSourceScanInput) {
  return apiRequest<ResearchSourceScanResp>("/ai/research-radar/scans", {
    method: "POST",
    body: JSON.stringify({
      topic: input.topic,
      ranking: input.ranking,
      limitPerSource: DEFAULT_LIMIT_PER_SOURCE,
      sources: researchSourcesForFilters(input.filters)
    })
  });
}

export function researchSourcesForFilters(filters: ResearchFilter[]): ResearchSource[] {
  const sources = new Set<ResearchSource>();

  if (filters.includes("papers")) {
    sources.add("arxiv");
    sources.add("paperswithcode");
  }
  if (filters.includes("projects")) {
    sources.add("github");
    sources.add("huggingface_models");
  }
  if (filters.includes("datasets")) {
    sources.add("huggingface_datasets");
  }
  if (filters.includes("benchmarks")) {
    sources.add("leaderboards");
  }

  if (sources.size === 0) {
    return [
      "arxiv",
      "paperswithcode",
      "github",
      "huggingface_models",
      "huggingface_datasets",
      "leaderboards"
    ];
  }

  return [...sources];
}
