"use client";

import {
  CheckCircle2,
  Copy,
  HardDrive,
  ListChecks,
  PackageCheck,
  Play,
  RefreshCw,
  Sparkles
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { toast } from "sonner";
import { generateCustomerPackage, listDeliveryTemplates } from "@/api/ai/template";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from "@/components/ui/select";
import type { CustomerPackageResp, DeliveryTemplateResp } from "@/types/ai-template";

const DEFAULT_TEMPLATE_CODE = "training_app";

interface InitForm {
  customerName: string;
  appName: string;
  industry: string;
  brandName: string;
  primaryColor: string;
  publicUrl: string;
}

const DEFAULT_FORM: InitForm = {
  customerName: "Acme",
  appName: "Acme Training",
  industry: "training",
  brandName: "Acme Academy",
  primaryColor: "#2563eb",
  publicUrl: "https://training.example.com"
};

export default function AiTemplatesPage() {
  const [templates, setTemplates] = useState<DeliveryTemplateResp[]>([]);
  const [selectedTemplateCode, setSelectedTemplateCode] = useState(DEFAULT_TEMPLATE_CODE);
  const [form, setForm] = useState<InitForm>(DEFAULT_FORM);
  const [packagePreview, setPackagePreview] = useState<CustomerPackageResp | null>(null);
  const [templateTotal, setTemplateTotal] = useState(0);
  const [templatesLoading, setTemplatesLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const selectedTemplate = useMemo(
    () => templates.find((template) => template.code === selectedTemplateCode) ?? templates[0] ?? null,
    [templates, selectedTemplateCode]
  );

  const loadTemplates = useCallback(async () => {
    setTemplatesLoading(true);
    try {
      const result = await listDeliveryTemplates({ page: 1, size: 20 });
      setTemplates(result.list);
      setTemplateTotal(result.total);
      const preferred = result.list.find((template) => template.code === selectedTemplateCode)
        ?? result.list.find((template) => template.code === DEFAULT_TEMPLATE_CODE)
        ?? result.list[0]
        ?? null;
      if (preferred) {
        setSelectedTemplateCode(preferred.code);
        applyTemplateDefaults(preferred);
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Delivery Template 加载失败");
    } finally {
      setTemplatesLoading(false);
    }
  }, [selectedTemplateCode]);

  useEffect(() => {
    void loadTemplates();
  }, [loadTemplates]);

  function applyTemplateDefaults(template: DeliveryTemplateResp) {
    setForm((current) => ({
      ...current,
      industry: template.category,
      brandName: template.branding.brandName,
      primaryColor: template.branding.primaryColor,
      publicUrl: template.branding.publicUrl
    }));
  }

  function selectTemplate(code: string) {
    setSelectedTemplateCode(code);
    setPackagePreview(null);
    const template = templates.find((item) => item.code === code);
    if (template) {
      applyTemplateDefaults(template);
    }
  }

  async function submitPackage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedTemplate) {
      toast.error("请选择交付模板");
      return;
    }
    setSubmitting(true);
    try {
      const result = await generateCustomerPackage({
        templateCode: selectedTemplate.code,
        customerName: form.customerName,
        appName: form.appName,
        industry: form.industry,
        brandName: form.brandName,
        primaryColor: form.primaryColor,
        publicUrl: form.publicUrl
      });
      setPackagePreview(result);
      toast.success(result.packageId);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Customer Package 生成失败");
    } finally {
      setSubmitting(false);
    }
  }

  async function copyPackageJson() {
    if (!packagePreview) {
      return;
    }
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error("Clipboard unavailable");
      }
      await navigator.clipboard.writeText(JSON.stringify(packagePreview, null, 2));
      toast.success("Package JSON copied");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Package JSON 复制失败");
    }
  }

  return (
    <div className="mx-auto grid w-full max-w-7xl items-start gap-4 xl:grid-cols-[380px_1fr]">
      <section className="rounded-lg border bg-background p-4">
        <div className="mb-3 flex items-center justify-between gap-3">
          <div className="min-w-0">
            <h1 className="truncate text-base font-semibold">Delivery Templates</h1>
            <p className="text-xs text-muted-foreground">{templateTotal} Templates</p>
          </div>
          <Button variant="outline" size="icon" title="刷新" onClick={() => void loadTemplates()} disabled={templatesLoading}>
            <RefreshCw />
          </Button>
        </div>

        <Select value={selectedTemplate?.code ?? selectedTemplateCode} onValueChange={selectTemplate}>
          <SelectTrigger>
            <SelectValue placeholder="选择模板" />
          </SelectTrigger>
          <SelectContent>
            {templates.map((template) => (
              <SelectItem key={template.code} value={template.code}>
                {template.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <div className="mt-4 grid gap-2">
          {templatesLoading ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">加载中</div>
          ) : null}
          {!templatesLoading && !templates.length ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">暂无模板</div>
          ) : null}
          {templates.map((template) => (
            <button
              key={template.code}
              type="button"
              className={[
                "grid w-full gap-2 rounded-md border p-3 text-left transition-colors hover:bg-muted/40",
                selectedTemplate?.code === template.code ? "border-primary bg-muted/45" : "bg-background"
              ].join(" ")}
              onClick={() => selectTemplate(template.code)}
            >
              <div className="flex min-w-0 items-center justify-between gap-2">
                <span className="truncate text-sm font-medium">{template.name}</span>
                <Badge variant="outline">{template.category}</Badge>
              </div>
              <p className="line-clamp-2 text-xs text-muted-foreground">{template.description}</p>
              <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                <span>{template.roles.length} Roles</span>
                <span>{template.frontendPages.length} Pages</span>
                <span>{template.smokeChecks.length} Smoke</span>
                <span>{template.evalSets.length} Evals</span>
              </div>
            </button>
          ))}
        </div>
      </section>

      <section className="grid gap-4">
        <form className="rounded-lg border bg-background p-4" onSubmit={(event) => void submitPackage(event)}>
          <div className="mb-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div className="min-w-0">
              <h2 className="truncate text-base font-semibold">Customer Init</h2>
              <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <span className="inline-flex items-center gap-1">
                  <HardDrive className="size-3.5" />
                  {selectedTemplate?.frontendApp ?? selectedTemplate?.frontendEntry ?? "-"}
                </span>
                <span className="inline-flex items-center gap-1">
                  <Sparkles className="size-3.5" />
                  {selectedTemplate?.branding.brandName ?? "-"}
                </span>
              </div>
            </div>
            <PermissionGate permissions={["ai:template:init"]}>
              <Button type="submit" disabled={!selectedTemplate || submitting}>
                <Play />
                Generate
              </Button>
            </PermissionGate>
          </div>

          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
            <Field label="Customer" value={form.customerName} onChange={(customerName) => setForm((current) => ({ ...current, customerName }))} />
            <Field label="App" value={form.appName} onChange={(appName) => setForm((current) => ({ ...current, appName }))} />
            <Field label="Industry" value={form.industry} onChange={(industry) => setForm((current) => ({ ...current, industry }))} />
            <Field label="Brand" value={form.brandName} onChange={(brandName) => setForm((current) => ({ ...current, brandName }))} />
            <div className="grid gap-1.5">
              <Label className="text-xs">Color</Label>
              <div className="grid grid-cols-[48px_1fr] gap-2">
                <Input
                  type="color"
                  className="h-9 p-1"
                  value={form.primaryColor}
                  onChange={(event) => setForm((current) => ({ ...current, primaryColor: event.target.value }))}
                />
                <Input
                  value={form.primaryColor}
                  onChange={(event) => setForm((current) => ({ ...current, primaryColor: event.target.value }))}
                />
              </div>
            </div>
            <Field label="Public URL" value={form.publicUrl} onChange={(publicUrl) => setForm((current) => ({ ...current, publicUrl }))} />
          </div>
        </form>

        <div className="grid gap-4 lg:grid-cols-[0.9fr_1.1fr]">
          <div className="rounded-lg border bg-background p-4">
            <div className="mb-3 flex items-center justify-between gap-3">
              <h2 className="text-sm font-medium">Package Contents</h2>
              <PackageCheck className="size-4 text-muted-foreground" />
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
              <Metric label="Roles" value={String(packagePreview?.roles.length ?? selectedTemplate?.roles.length ?? "-")} />
              <Metric label="Pages" value={String(packagePreview?.frontendPages.length ?? selectedTemplate?.frontendPages.length ?? "-")} />
              <Metric label="Capabilities" value={String(packagePreview ? capabilityCount(packagePreview) : selectedTemplate ? templateCapabilityCount(selectedTemplate) : "-")} />
              <Metric label="Smoke" value={String(packagePreview?.smokeChecks.length ?? selectedTemplate?.smokeChecks.length ?? "-")} />
            </div>

            <div className="mt-4 grid gap-2">
              {(packagePreview?.roles ?? selectedTemplate?.roles ?? []).map((role) => (
                <div key={role.code} className="rounded-md border p-3">
                  <div className="flex min-w-0 items-center justify-between gap-2">
                    <span className="truncate text-sm font-medium">{role.name}</span>
                    <Badge variant="outline">{role.permissions.length}</Badge>
                  </div>
                  <p className="mt-1 truncate text-xs text-muted-foreground">{role.code}</p>
                </div>
              ))}
            </div>
          </div>

          <div className="rounded-lg border bg-background p-4">
            <div className="mb-3 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <h2 className="truncate text-sm font-medium">{packagePreview?.packageId ?? "Deployment Preview"}</h2>
                <p className="text-xs text-muted-foreground">{packagePreview?.tenantConfig.frontendApp ?? selectedTemplate?.frontendApp ?? "-"}</p>
              </div>
              {packagePreview ? (
                <Button type="button" variant="outline" size="sm" onClick={() => void copyPackageJson()}>
                  <Copy />
                  Copy JSON
                </Button>
              ) : (
                <ListChecks className="size-4 text-muted-foreground" />
              )}
            </div>

            <div className="grid gap-3">
              <CapabilityGroup title="Connectors" items={(packagePreview?.connectors ?? selectedTemplate?.connectors ?? []).map((item) => item.name)} />
              <CapabilityGroup title="Plugins" items={(packagePreview?.plugins ?? selectedTemplate?.plugins ?? []).map((item) => item.name)} />
              <CapabilityGroup title="Triggers" items={(packagePreview?.triggers ?? selectedTemplate?.triggers ?? []).map((item) => item.name)} />
              <CapabilityGroup title="Eval Sets" items={(packagePreview?.evalSets ?? selectedTemplate?.evalSets ?? []).map((item) => `${item.name} (${item.caseCount})`)} />
              <CapabilityGroup title="Smoke" items={(packagePreview?.smokeChecks ?? selectedTemplate?.smokeChecks ?? []).map((item) => `${item.workdir}: ${item.command}`)} />
            </div>

            <div className="mt-4 grid gap-2">
              {(packagePreview?.deploymentSteps ?? selectedTemplate?.deploymentChecklist ?? []).map((step) => (
                <div key={step} className="flex items-start gap-2 rounded-md border p-3 text-sm">
                  <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-primary" />
                  <span>{step}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

function Field({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <div className="grid gap-1.5">
      <Label className="text-xs">{label}</Label>
      <Input value={value} onChange={(event) => onChange(event.target.value)} />
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md border bg-muted/20 p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 truncate text-sm font-medium">{value}</div>
    </div>
  );
}

function CapabilityGroup({ title, items }: { title: string; items: string[] }) {
  return (
    <div className="rounded-md border p-3">
      <div className="mb-2 flex items-center justify-between gap-2">
        <span className="text-xs font-medium">{title}</span>
        <Badge variant="outline">{items.length}</Badge>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {items.length ? items.map((item) => <Badge key={item} variant="secondary">{item}</Badge>) : (
          <span className="text-xs text-muted-foreground">-</span>
        )}
      </div>
    </div>
  );
}

function templateCapabilityCount(template: DeliveryTemplateResp) {
  return template.connectors.length + template.plugins.length + template.triggers.length + template.skills.length;
}

function capabilityCount(pkg: CustomerPackageResp) {
  return pkg.connectors.length + pkg.plugins.length + pkg.triggers.length + pkg.skills.length;
}
