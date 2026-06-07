import { describe, expect, it, vi } from "vitest";
import { listTrainingLearningRecords } from "./training";
import { api } from "@/lib/api";

vi.mock("@/lib/api", () => ({
  api: {
    get: vi.fn()
  }
}));

describe("ai training api", () => {
  it("lists tenant learning records for HR training review", async () => {
    vi.mocked(api.get).mockResolvedValue({
      scope: "tenant",
      summary: {
        completionRate: 72,
        pendingTaskCount: 2,
        quizAverageScore: 91,
        weakPointCount: 2
      },
      tasks: [],
      records: [],
      weakPoints: []
    });

    await listTrainingLearningRecords({ scope: "tenant" });

    expect(api.get).toHaveBeenCalledWith("/ai/training/learning-records", {
      scope: "tenant"
    });
  });
});
