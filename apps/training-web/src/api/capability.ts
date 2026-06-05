import { apiRequest } from "@/lib/api";
import type { ToolDryRunCommand, ToolDryRunResp } from "@/types/capability";

const TOOL_DRY_RUN_URL = "/ai/capabilities/tools/dry-run";

export function dryRunTool(data: ToolDryRunCommand) {
  return apiRequest<ToolDryRunResp>(TOOL_DRY_RUN_URL, {
    method: "POST",
    body: data
  });
}
