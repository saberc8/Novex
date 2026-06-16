import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const routes = JSON.parse(readFileSync(join(here, "template-routes.json"), "utf8"));

const requiredRoutes = new Map([
  ["/customer-service", "ai:customer-service:agent:run"],
  ["/customer-service/runs", "ai:customer-service:agent:list"],
  ["/customer-service/knowledge", "ai:customer-service:read"],
]);

assert.equal(routes.length, requiredRoutes.size);

for (const [path, permission] of requiredRoutes.entries()) {
  const route = routes.find((item) => item.path === path);
  assert.ok(route, `missing route ${path}`);
  assert.equal(route.permission, permission);
}

console.log("customer-service-agent template routes ok");
