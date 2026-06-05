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
}

export interface CustomerPackageResp {
  packageId: string;
  template: DeliveryTemplateResp;
  tenantConfig: CustomerTenantConfig;
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
  deploymentSteps: string[];
}
