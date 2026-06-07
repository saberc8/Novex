import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  CustomerPackageApplyResp,
  CustomerPackageCommand,
  CustomerPackageResp,
  DeliveryTemplateQuery,
  DeliveryTemplateResp,
  TemplateSmokeRunCommand,
  TemplateSmokeRunResp
} from "@/types/ai-template";

const TEMPLATE_URL = "/ai/templates";

export function listDeliveryTemplates(query: DeliveryTemplateQuery = {}) {
  return api.get<PageResult<DeliveryTemplateResp>>(TEMPLATE_URL, { ...query });
}

export function getDeliveryTemplate(code: string) {
  return api.get<DeliveryTemplateResp>(`${TEMPLATE_URL}/${code}`);
}

export function generateCustomerPackage(data: CustomerPackageCommand) {
  return api.post<CustomerPackageResp>(`${TEMPLATE_URL}/packages`, data);
}

export function applyCustomerPackage(data: CustomerPackageCommand) {
  return api.post<CustomerPackageApplyResp>(`${TEMPLATE_URL}/packages/apply`, data);
}

export function runTemplateSmoke(data: TemplateSmokeRunCommand) {
  return api.post<TemplateSmokeRunResp>(`${TEMPLATE_URL}/smoke/runs`, data);
}
