import { existsSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

describe("agent-workspace template routes", () => {
  it("has Next page routes for the agent workspace template entry points", () => {
    const appDir = path.join(process.cwd(), "app");

    for (const route of ["/", "/agent", "/agent/approvals", "/agent/traces"]) {
      const routeFile =
        route === "/"
          ? path.join(appDir, "page.tsx")
          : path.join(appDir, route.slice(1), "page.tsx");

      expect(existsSync(routeFile), `${route} route missing`).toBe(true);
    }
  });
});
