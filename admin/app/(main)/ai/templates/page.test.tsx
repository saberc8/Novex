import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiTemplatesPage from "./page";
import { applyCustomerPackage, generateCustomerPackage, listDeliveryTemplates, runTemplateSmoke } from "@/api/ai/template";
import type { CustomerPackageApplyResp, CustomerPackageResp, DeliveryTemplateResp, TemplateSmokeRunResp } from "@/types/ai-template";

vi.mock("@/api/ai/template", () => ({
  applyCustomerPackage: vi.fn(),
  generateCustomerPackage: vi.fn(),
  listDeliveryTemplates: vi.fn(),
  runTemplateSmoke: vi.fn()
}));

vi.mock("@/components/permission/permission-gate", () => ({
  PermissionGate: ({ children }: { children: ReactNode }) => <>{children}</>
}));

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn()
  }
}));

const generateCustomerPackageMock = vi.mocked(generateCustomerPackage);
const applyCustomerPackageMock = vi.mocked(applyCustomerPackage);
const listDeliveryTemplatesMock = vi.mocked(listDeliveryTemplates);
const runTemplateSmokeMock = vi.mocked(runTemplateSmoke);
const writeTextMock = vi.fn<(text: string) => Promise<void>>(() => Promise.resolve());

function trainingTemplate(): DeliveryTemplateResp {
  return {
    code: "training_app",
    name: "Training App",
    category: "training",
    description: "Employee training template",
    frontendEntry: "apps/training-web",
    frontendApp: "training-web",
    frontendPages: [
      {
        code: "learn",
        title: "Learning Tasks",
        path: "/",
        navLabel: "学习",
        permission: "app:training:learn"
      }
    ],
    smokeChecks: [
      {
        code: "training_web_unit",
        name: "Training Web Unit Tests",
        workdir: "apps/training-web",
        command: "pnpm test"
      }
    ],
    smokeScript: "templates/training-app/smoke.sh",
    sort: 4,
    status: 1,
    branding: {
      brandName: "Novex Academy",
      logoText: "TA",
      primaryColor: "#db2777",
      publicUrl: "https://training.example.com"
    },
    roles: [
      {
        code: "training_admin",
        name: "Training Admin",
        permissions: ["ai:template:init"]
      }
    ],
    menus: [
      {
        code: "training.home",
        title: "Training",
        path: "/training",
        permission: "app:training:learn"
      }
    ],
    prompts: [],
    skills: [
      {
        code: "training_quiz",
        name: "Training Quiz",
        description: "Builds quizzes from cited training content."
      }
    ],
    connectors: [],
    plugins: [],
    triggers: [],
    evalSets: [
      {
        code: "training_regression",
        name: "Training Regression",
        caseCount: 20,
        metrics: ["citation_accuracy"]
      }
    ],
    deploymentChecklist: ["Run training_regression eval set before pilot"]
  };
}

function customerPackage(): CustomerPackageResp {
  const template = trainingTemplate();
  return {
    packageId: "pkg_training_app_acme",
    template,
    tenantConfig: {
      customerName: "Acme",
      appName: "Acme Training",
      industry: "training",
      templateCode: "training_app",
      frontendEntry: "apps/training-web",
      frontendApp: "training-web"
    },
    branding: {
      brandName: "Acme Academy",
      logoText: "TA",
      primaryColor: "#2563eb",
      publicUrl: "https://training.example.com"
    },
    frontendConfig: {
      app: "training-web",
      entry: "apps/training-web",
      entryUrl: "https://training.example.com",
      branding: {
        brandName: "Acme Academy",
        logoText: "TA",
        primaryColor: "#2563eb",
        publicUrl: "https://training.example.com"
      },
      defaultPage: {
        code: "learn",
        title: "Learning Tasks",
        path: "/",
        navLabel: "学习",
        permission: "app:training:learn"
      },
      navigation: template.frontendPages,
      allowedRoles: [
        {
          code: "learner",
          name: "Learner",
          permissions: ["app:training:learn", "app:training:ask"]
        }
      ]
    },
    provisioningPlan: {
      planId: "prov_pkg_training_app_acme",
      mode: "operator_applied",
      tenantCode: "acme",
      idempotencyKey: "training_app:acme",
      steps: [
        {
          code: "tenant",
          title: "Create or update tenant",
          target: "sys_tenant",
          operation: "upsert",
          payload: {
            tenantCode: "acme"
          }
        },
        {
          code: "roles",
          title: "Create roles and bind permissions",
          target: "sys_role",
          operation: "upsert_and_bind",
          payload: {
            roles: template.roles
          }
        }
      ]
    },
    roles: template.roles,
    menus: template.menus,
    frontendPages: template.frontendPages,
    prompts: template.prompts,
    skills: template.skills,
    connectors: template.connectors,
    plugins: template.plugins,
    triggers: template.triggers,
    evalSets: template.evalSets,
    deploymentChecklist: template.deploymentChecklist,
    smokeScript: template.smokeScript,
    smokeChecks: template.smokeChecks,
    deploymentSteps: ["Initialize customer Acme from template training_app"]
  };
}

function customerPackageApply(): CustomerPackageApplyResp {
  const pkg = customerPackage();
  return {
    package: pkg,
    tenantId: 42,
    tenantCode: "acme",
    appliedSteps: pkg.provisioningPlan.steps,
    pendingOperatorSteps: [
      {
        code: "frontend",
        title: "Apply branding and frontend publish config",
        target: "frontend_config",
        operation: "upsert",
        payload: {}
      }
    ]
  };
}

function templateSmokeRun(): TemplateSmokeRunResp {
  return {
    runId: 9001,
    templateCode: "training_app",
    packageId: "pkg_training_app_acme",
    smokeScript: "templates/training-app/smoke.sh",
    status: "planned",
    dryRun: true,
    totalChecks: 1,
    passedChecks: 0,
    failedChecks: 0,
    checks: [
      {
        code: "training_web_unit",
        name: "Training Web Unit Tests",
        workdir: "apps/training-web",
        command: "pnpm test",
        status: "planned",
        exitCode: null,
        stdout: "",
        stderr: "",
        durationMs: 0
      }
    ]
  };
}

describe("AiTemplatesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(window.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: writeTextMock
      }
    });
    listDeliveryTemplatesMock.mockResolvedValue({
      list: [trainingTemplate()],
      total: 1
    });
    generateCustomerPackageMock.mockResolvedValue(customerPackage());
    applyCustomerPackageMock.mockResolvedValue(customerPackageApply());
    runTemplateSmokeMock.mockResolvedValue(templateSmokeRun());
  });

  it("generates a customer package and copies the delivery JSON", async () => {
    render(<AiTemplatesPage />);

    expect((await screen.findAllByText("Training App")).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() =>
      expect(generateCustomerPackageMock).toHaveBeenCalledWith({
        templateCode: "training_app",
        customerName: "Acme",
        appName: "Acme Training",
        industry: "training",
        brandName: "Novex Academy",
        primaryColor: "#db2777",
        publicUrl: "https://training.example.com"
      })
    );
    expect(await screen.findByText("pkg_training_app_acme")).toBeTruthy();
    expect(await screen.findByText("templates/training-app/smoke.sh")).toBeTruthy();
    expect(await screen.findByText("Frontend Publish")).toBeTruthy();
    expect(await screen.findByText("Provisioning Plan")).toBeTruthy();
    expect(await screen.findByText("sys_tenant: upsert")).toBeTruthy();
    expect(await screen.findByText("https://training.example.com")).toBeTruthy();
    expect(await screen.findByText("Default: /")).toBeTruthy();
    expect(await screen.findByText("Learner")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Copy JSON" }));

    await waitFor(() => expect(writeTextMock).toHaveBeenCalledTimes(1));
    const copied = writeTextMock.mock.calls[0]?.[0] ?? "";
    expect(copied).toContain("\"packageId\": \"pkg_training_app_acme\"");
    expect(copied).toContain("\"templateCode\": \"training_app\"");
    expect(copied).toContain("\"frontendConfig\"");
    expect(copied).toContain("\"provisioningPlan\"");

    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(applyCustomerPackageMock).toHaveBeenCalledWith({
        templateCode: "training_app",
        customerName: "Acme",
        appName: "Acme Training",
        industry: "training",
        brandName: "Novex Academy",
        primaryColor: "#db2777",
        publicUrl: "https://training.example.com"
      })
    );
    expect(await screen.findByText("tenant acme")).toBeTruthy();
    expect(await screen.findByText("Applied Steps")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Plan Smoke" }));
    await waitFor(() =>
      expect(runTemplateSmokeMock).toHaveBeenCalledWith({
        templateCode: "training_app",
        packageId: "pkg_training_app_acme",
        dryRun: true
      })
    );
    expect(await screen.findByText("Smoke Result")).toBeTruthy();
    expect(await screen.findByText("run #9001 planned")).toBeTruthy();
  });
});
