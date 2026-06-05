import { CapabilityRegistry } from "@/components/ai/capability-registry";

export default function AiTriggersPage() {
  return <CapabilityRegistry title="触发器" resource="triggers" permission="ai:trigger:list" />;
}
