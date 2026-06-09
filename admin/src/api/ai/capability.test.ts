import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  dryRunTool,
  getCapabilitySummary,
  importSkill,
  importSkillFromSource,
  importSkillPackage,
  installPlugin,
  listConnectorCredentials,
  listConnectors,
  listMcpServers,
  listPluginInstallations,
  listPlugins,
  listSkills,
  listToolAudits,
  listTools,
  listTriggers,
  previewSkillImport,
  upsertMcpServer,
  upsertConnectorCredential
} from "@/api/ai/capability";

function okResponse(data: unknown = true) {
  return Promise.resolve(
    new Response(
      JSON.stringify({
        code: "200",
        data,
        msg: "成功",
        success: true,
        timestamp: "1"
      }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" }
      }
    )
  );
}

describe("capability api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses capability summary, registry, dry-run, and audit endpoints", async () => {
    await getCapabilitySummary();
    await listSkills({ page: 1, size: 10 });
    await listTools({ page: 2, size: 20, kind: "media" });
    await listConnectors({ status: 1 });
    await listConnectorCredentials({ connectorCode: "github.default" });
    await listPlugins();
    await listPluginInstallations({ pluginCode: "builtin.github-basic", enabled: true });
    await listTriggers();
    await listMcpServers({ kind: "tenant" });
    await upsertConnectorCredential({
      connectorCode: "github.default",
      scopeType: "tenant",
      scopeId: "1",
      authType: "oauth_app",
      secretRef: "env:GITHUB_CONNECTOR_TOKEN",
      scopes: ["repo"],
      status: 1
    });
    await installPlugin({
      pluginCode: "builtin.github-basic",
      version: "0.1.0",
      enabled: true,
      permissionGrants: ["ai:connector:list", "ai:tool:dryRun"],
      config: {}
    });
    await upsertMcpServer({
      code: "docs.search",
      name: "Docs Search",
      endpointUrl: "https://mcp.example.com/sse",
      transportKind: "streamable_http",
      authScope: "tenant",
      authType: "bearer_env",
      secretRef: "env:DOCS_MCP_TOKEN",
      networkAllowlist: ["mcp.example.com"],
      toolAllowlist: ["docs.search"],
      discoveredTools: [],
      enabled: true
    });
    await dryRunTool({ toolCode: "rag.search", input: { query: "hello" } });
    await listToolAudits({ page: 1, size: 5, toolCode: "rag.search" });

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:4398/ai/capabilities/summary");
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/skills?page=1&size=10"
    );
    expect(fetchMock.mock.calls[2]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/tools?page=2&size=20&kind=media"
    );
    expect(fetchMock.mock.calls[3]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/connectors?status=1"
    );
    expect(fetchMock.mock.calls[4]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/connectors/credentials?connectorCode=github.default"
    );
    expect(fetchMock.mock.calls[5]?.[0]).toBe("http://localhost:4398/ai/capabilities/plugins");
    expect(fetchMock.mock.calls[6]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/plugins/installations?pluginCode=builtin.github-basic&enabled=true"
    );
    expect(fetchMock.mock.calls[7]?.[0]).toBe("http://localhost:4398/ai/capabilities/triggers");
    expect(fetchMock.mock.calls[8]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/mcp-servers?kind=tenant"
    );
    expect(fetchMock.mock.calls[9]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/connectors/credentials"
    );
    expect(fetchMock.mock.calls[9]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        connectorCode: "github.default",
        scopeType: "tenant",
        scopeId: "1",
        authType: "oauth_app",
        secretRef: "env:GITHUB_CONNECTOR_TOKEN",
        scopes: ["repo"],
        status: 1
      })
    });
    expect(fetchMock.mock.calls[10]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/plugins/installations"
    );
    expect(fetchMock.mock.calls[10]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        pluginCode: "builtin.github-basic",
        version: "0.1.0",
        enabled: true,
        permissionGrants: ["ai:connector:list", "ai:tool:dryRun"],
        config: {}
      })
    });
    expect(fetchMock.mock.calls[11]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/mcp-servers"
    );
    expect(fetchMock.mock.calls[11]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        code: "docs.search",
        name: "Docs Search",
        endpointUrl: "https://mcp.example.com/sse",
        transportKind: "streamable_http",
        authScope: "tenant",
        authType: "bearer_env",
        secretRef: "env:DOCS_MCP_TOKEN",
        networkAllowlist: ["mcp.example.com"],
        toolAllowlist: ["docs.search"],
        discoveredTools: [],
        enabled: true
      })
    });
    expect(fetchMock.mock.calls[12]?.[0]).toBe("http://localhost:4398/ai/capabilities/tools/dry-run");
    expect(fetchMock.mock.calls[12]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ toolCode: "rag.search", input: { query: "hello" } })
    });
    expect(fetchMock.mock.calls[13]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/tools/audits?page=1&size=5&toolCode=rag.search"
    );
  });

  it("uploads skill imports as multipart form data", async () => {
    const formData = new FormData();
    formData.append("file", new File(["# Skill"], "SKILL.md", { type: "text/markdown" }));

    await importSkill(formData);

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/skills/import"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      body: formData
    });
  });

  it("uses skill AI import preview, source install, and package endpoints", async () => {
    await previewSkillImport({
      source: "https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer"
    });
    await importSkillFromSource({
      source: "https://github.com/KKKKhazix/khazix-skills",
      skillPath: "khazix-writer"
    });
    const packageData = new FormData();
    packageData.append("file", new File(["zip"], "khazix-writer.zip", { type: "application/zip" }));
    await importSkillPackage(packageData);

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/skills/import/preview"
    );
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        source: "https://github.com/KKKKhazix/khazix-skills/tree/main/khazix-writer"
      })
    });
    expect(fetchMock.mock.calls[1]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/skills/import/source"
    );
    expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        source: "https://github.com/KKKKhazix/khazix-skills",
        skillPath: "khazix-writer"
      })
    });
    expect(fetchMock.mock.calls[2]?.[0]).toBe(
      "http://localhost:4398/ai/capabilities/skills/import/package"
    );
    expect(fetchMock.mock.calls[2]?.[1]).toMatchObject({
      method: "POST",
      body: packageData
    });
  });
});
