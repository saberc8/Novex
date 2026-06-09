import { existsSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

describe("chat-web template routes", () => {
  it("has Next page routes for the chat and knowledge template entry points", () => {
    const appDir = path.join(process.cwd(), "app");

    for (const route of [
      "/",
      "/chat",
      "/chat/history",
      "/knowledge",
      "/knowledge/[datasetId]",
      "/knowledge/sources",
      "/knowledge/sources/[datasetId]",
      "/settings",
      "/share/[token]"
    ]) {
      const routeFile =
        route === "/"
          ? path.join(appDir, "page.tsx")
          : path.join(appDir, route.slice(1), "page.tsx");

      expect(existsSync(routeFile), `${route} route missing`).toBe(true);
    }
  });
});
