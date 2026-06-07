import { existsSync } from "node:fs";
import path from "node:path";
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

  it("has a Next page route for every customer navigation target", () => {
    const appDir = path.join(process.cwd(), "app");

    for (const item of trainingNavItems) {
      const routeFile =
        item.href === "/"
          ? path.join(appDir, "page.tsx")
          : path.join(appDir, item.href.slice(1), "page.tsx");

      expect(existsSync(routeFile), `${item.href} route missing`).toBe(true);
    }
  });
});
