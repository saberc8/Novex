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

  it("posts topic planner search hints to the backend scan endpoint", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: {
          topic: "量化因子",
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
      topic: "量化因子",
      filters: ["papers", "projects", "datasets"],
      ranking: "balanced",
      topicPlan: {
        topic: "量化因子",
        summary: "用金融数据构造并验证 alpha 信号。",
        domains: ["金融工程"],
        learningGoals: ["理解因子定义"],
        keyConcepts: ["alpha factor"],
        searchQueries: ["quant factor investing", "qlib factor research"],
        relevanceKeywords: ["factor", "alpha", "backtest", "IC"],
        sourcePriorities: ["papers", "projects", "datasets"]
      }
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/research-radar/scans",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          topic: "量化因子",
          ranking: "balanced",
          limitPerSource: 5,
          sources: ["arxiv", "paperswithcode", "github", "huggingface_models", "huggingface_datasets"],
          searchQueries: ["quant factor investing", "qlib factor research"],
          relevanceKeywords: ["factor", "alpha", "backtest", "IC"]
        })
      })
    );
  });
});
