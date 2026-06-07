import { CapabilityRegistry } from "@/components/ai/capability-registry";
import { ConnectorCredentialPanel } from "@/components/ai/connector-credential-panel";

export default function AiConnectorsPage() {
  return (
    <div className="grid gap-4">
      <CapabilityRegistry title="连接器" resource="connectors" permission="ai:connector:list" />
      <ConnectorCredentialPanel />
    </div>
  );
}
