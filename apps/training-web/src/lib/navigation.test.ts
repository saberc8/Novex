import { describe, expect, it } from "vitest";
import { trainingNavItems } from "./navigation";

describe("training navigation", () => {
  it("keeps the POC customer app sections in the expected order", () => {
    expect(trainingNavItems.map((item) => item.href)).toEqual([
      "/",
      "/ask",
      "/quiz",
      "/records",
      "/notifications"
    ]);
  });
});
