import { beforeEach, describe, expect, it, vi } from "vitest";
import { listTrainingLearningRecords } from "./training";

const fetchMock = vi.fn();

vi.stubGlobal("fetch", fetchMock);

describe("training learning api", () => {
  beforeEach(() => {
    fetchMock.mockReset();
    window.localStorage.clear();
    window.localStorage.setItem("novex_token", "token-1");
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({
        code: "200",
        msg: "success",
        success: true,
        timestamp: "2026-06-05T12:00:00Z",
        data: {
          scope: "self",
          summary: {
            completionRate: 72,
            pendingTaskCount: 2,
            quizAverageScore: 86,
            weakPointCount: 2
          },
          tasks: [],
          records: [],
          weakPoints: []
        }
      })
    });
  });

  it("loads employee learning records and weak points", async () => {
    await listTrainingLearningRecords({ scope: "self" });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/training/learning-records?scope=self"
    );
    const init = fetchMock.mock.calls[0]?.[1] as RequestInit;
    expect((init.headers as Record<string, string>).Authorization).toBe("Bearer token-1");
  });
});
