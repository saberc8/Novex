import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AiTemplatesPage from "./page";
import { generateCustomerPackage, listDeliveryTemplates } from "@/api/ai/template";
import type { CustomerPackageResp, DeliveryTemplateResp } from "@/types/ai-template";

vi.mock("@/api/ai/template", () => ({
  generateCustomerPackage: vi.fn(),
  listDeliveryTemplates: vi.fn()
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
const listDeliveryTemplatesMock = vi.mocked(listDeliveryTemplates);
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
    smokeChecks: template.smokeChecks,
    deploymentSteps: ["Initialize customer Acme from template training_app"]
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

    fireEvent.click(screen.getByRole("button", { name: "Copy JSON" }));

    await waitFor(() => expect(writeTextMock).toHaveBeenCalledTimes(1));
    const copied = writeTextMock.mock.calls[0]?.[0] ?? "";
    expect(copied).toContain("\"packageId\": \"pkg_training_app_acme\"");
    expect(copied).toContain("\"templateCode\": \"training_app\"");
  });
});
