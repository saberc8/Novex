import { apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type { CapabilityItemResp, CapabilityQuery } from "@/types/capability";

const CAPABILITY_SKILLS_URL = "/ai/capabilities/skills";

export function listSkills(query: CapabilityQuery = {}) {
  return apiRequest<PageResult<CapabilityItemResp>>(CAPABILITY_SKILLS_URL, {
    query
  });
}
