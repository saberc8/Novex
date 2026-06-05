export interface ToolDryRunCommand {
  toolCode: string;
  input?: Record<string, unknown>;
}

export interface ToolDryRunResp {
  auditId: number;
  toolCode: string;
  status: string;
  dryRun: boolean;
  response: Record<string, unknown> | unknown[] | string | number | boolean | null;
}
