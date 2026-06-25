import { describe, expect, it, vi } from "vitest";
import { createResearchRadarSourceScan } from "./source-scan";

describe("research radar source scan api", () => {
  it("posts selected source filters to the backend scan endpoint", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: {
          topic: "agent workflow",
          ranking: "balanced",
          status: "succeeded",
          sources: [],
          items: [],
          promptContext: "Research Radar Evidence",
          warnings: []
        }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_API_BASE_URL", "http://localhost:62601");

    await createResearchRadarSourceScan({
      topic: "agent workflow",
      filters: ["papers", "projects", "datasets", "benchmarks"],
      ranking: "balanced"
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/research-radar/scans",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          topic: "agent workflow",
          ranking: "balanced",
          limitPerSource: 5,
          sources: [
            "arxiv",
            "paperswithcode",
            "github",
            "huggingface_models",
            "huggingface_datasets",
            "leaderboards"
          ]
        })
      })
    );
  });
});
