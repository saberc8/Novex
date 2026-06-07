import { CapabilityRegistry } from "@/components/ai/capability-registry";

export default function AiSkillsPage() {
  return <CapabilityRegistry title="Skills" resource="skills" permission="ai:skill:list" />;
}
