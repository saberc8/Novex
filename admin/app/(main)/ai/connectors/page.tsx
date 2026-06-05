import { CapabilityRegistry } from "@/components/ai/capability-registry";

export default function AiConnectorsPage() {
  return <CapabilityRegistry title="连接器" resource="connectors" permission="ai:connector:list" />;
}
