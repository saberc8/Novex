import type { PageQuery } from "@/types/api";

export interface DeliveryTemplateQuery extends PageQuery {
  status?: number;
  category?: string;
}

export interface TemplateBranding {
  brandName: string;
  logoText: string;
  primaryColor: string;
  publicUrl: string;
}

export interface TemplateFrontendPage {
  code: string;
  title: string;
  path: string;
  navLabel: string;
  permission: string;
}

export interface TemplateSmokeCheck {
  code: string;
  name: string;
  workdir: string;
  command: string;
}

export interface TemplateRole {
  code: string;
  name: string;
  permissions: string[];
}

export interface TemplateMenu {
  code: string;
  title: string;
  path: string;
  permission: string;
}

export interface TemplatePrompt {
  code: string;
  name: string;
  content: string;
}

export interface TemplateSkill {
  code: string;
  name: string;
  description: string;
}

export interface TemplateConnector {
  code: string;
  name: string;
  kind: string;
  authType: string;
}

export interface TemplatePlugin {
  code: string;
  name: string;
  runtime: string;
}

export interface TemplateTrigger {
  code: string;
  name: string;
  sourceType: string;
  target: string;
}

export interface TemplateEvalSet {
  code: string;
  name: string;
  caseCount: number;
  metrics: string[];
}

export interface DeliveryTemplateResp {
  code: string;
  name: string;
  category: string;
  description: string;
  frontendEntry: string;
  frontendApp: string;
  frontendPages: TemplateFrontendPage[];
  smokeChecks: TemplateSmokeCheck[];
  smokeScript: string;
  sort: number;
  status: number;
  branding: TemplateBranding;
  roles: TemplateRole[];
  menus: TemplateMenu[];
  prompts: TemplatePrompt[];
  skills: TemplateSkill[];
  connectors: TemplateConnector[];
  plugins: TemplatePlugin[];
  triggers: TemplateTrigger[];
  evalSets: TemplateEvalSet[];
  deploymentChecklist: string[];
}

export interface CustomerPackageCommand {
  templateCode: string;
  customerName: string;
  appName: string;
  industry?: string;
  brandName?: string;
  primaryColor?: string;
  publicUrl?: string;
}

export interface CustomerTenantConfig {
  customerName: string;
  appName: string;
  industry: string;
  templateCode: string;
  frontendEntry: string;
  frontendApp: string;
}

export interface CustomerFrontendConfig {
  app: string;
  entry: string;
  entryUrl: string;
  branding: TemplateBranding;
  defaultPage: TemplateFrontendPage;
  navigation: TemplateFrontendPage[];
  allowedRoles: TemplateRole[];
}

export interface CustomerProvisioningStep {
  code: string;
  title: string;
  target: string;
  operation: string;
  payload: Record<string, unknown>;
}

export interface CustomerProvisioningPlan {
  planId: string;
  mode: string;
  tenantCode: string;
  idempotencyKey: string;
  steps: CustomerProvisioningStep[];
}

export interface CustomerPackageResp {
  packageId: string;
  template: DeliveryTemplateResp;
  tenantConfig: CustomerTenantConfig;
  branding: TemplateBranding;
  frontendConfig: CustomerFrontendConfig;
  provisioningPlan: CustomerProvisioningPlan;
  roles: TemplateRole[];
  menus: TemplateMenu[];
  frontendPages: TemplateFrontendPage[];
  prompts: TemplatePrompt[];
  skills: TemplateSkill[];
  connectors: TemplateConnector[];
  plugins: TemplatePlugin[];
  triggers: TemplateTrigger[];
  evalSets: TemplateEvalSet[];
  deploymentChecklist: string[];
  smokeScript: string;
  smokeChecks: TemplateSmokeCheck[];
  deploymentSteps: string[];
}

export interface CustomerPackageApplyResp {
  package: CustomerPackageResp;
  tenantId: number;
  tenantCode: string;
  appliedSteps: CustomerProvisioningStep[];
  pendingOperatorSteps: CustomerProvisioningStep[];
}

export interface TemplateSmokeRunCommand {
  templateCode: string;
  packageId?: string;
  dryRun?: boolean;
}

export interface TemplateSmokeCheckRunResp {
  code: string;
  name: string;
  workdir: string;
  command: string;
  status: string;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  durationMs: number;
}

export interface TemplateSmokeRunResp {
  runId: number;
  templateCode: string;
  packageId?: string | null;
  smokeScript: string;
  status: string;
  dryRun: boolean;
  totalChecks: number;
  passedChecks: number;
  failedChecks: number;
  checks: TemplateSmokeCheckRunResp[];
}
