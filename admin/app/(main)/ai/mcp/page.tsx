"use client";

import { useState } from "react";
import { CapabilityRegistry } from "@/components/ai/capability-registry";
import { McpServerPanel } from "@/components/ai/mcp-server-panel";

export default function AiMcpPage() {
  const [registryVersion, setRegistryVersion] = useState(0);

  return (
    <div className="grid gap-4">
      <CapabilityRegistry
        key={registryVersion}
        title="MCP"
        resource="mcpServers"
        permission="ai:mcp:list"
      />
      <McpServerPanel onSaved={() => setRegistryVersion((version) => version + 1)} />
    </div>
  );
}
