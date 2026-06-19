import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  deleteModelRegistryRoute,
  getModelRegistry,
  getModelRuntimeConfig,
  runModelHealthCheck,
  upsertModelRegistryRoute
} from "@/api/ai/model";
import type { ModelRegistrySummary, ModelRuntimeSummary } from "@/types/ai-model";
import AiModelsPage from "./page";

vi.mock("@/api/ai/model", () => ({
  deleteModelRegistryRoute: vi.fn(),
  getModelRegistry: vi.fn(),
  getModelRuntimeConfig: vi.fn(),
  runModelHealthCheck: vi.fn(),
  upsertModelRegistryRoute: vi.fn()
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

const deleteModelRegistryRouteMock = vi.mocked(deleteModelRegistryRoute);
const getModelRegistryMock = vi.mocked(getModelRegistry);
const getModelRuntimeConfigMock = vi.mocked(getModelRuntimeConfig);
const runModelHealthCheckMock = vi.mocked(runModelHealthCheck);
const upsertModelRegistryRouteMock = vi.mocked(upsertModelRegistryRoute);

const runtimeSummary: ModelRuntimeSummary = {
  routes: [
    {
      target: "llm",
      routeId: "runtime.llm.chat",
      kind: "llm",
      provider: "deep-seek",
      model: "deepseek-v4-flash",
      baseUrl: "https://api.deepseek.com",
      endpoint: "https://api.deepseek.com/chat/completions",
      maskedApiKey: "configured",
      purposes: ["chat"],
      envKeys: ["LLM_API_KEY"],
      purposeRouteIds: { chat: "runtime.llm.chat" }
    }
  ],
  missingEnv: []
};

const registrySummary: ModelRegistrySummary = {
  providerCount: 1,
  deploymentCount: 1,
  profileCount: 1,
  routeCount: 1,
  providers: [
    {
      id: 1,
      code: "deepseek",
      name: "DeepSeek",
      providerType: "deep-seek",
      status: 1
    }
  ],
  deployments: [
    {
      id: 1,
      providerId: 1,
      code: "deepseek-public",
      name: "DeepSeek Public API",
      endpoint: "https://api.deepseek.com",
      apiPath: "/chat/completions",
      networkZone: "public",
      status: 1
    }
  ],
  profiles: [
    {
      id: 1,
      deploymentId: 1,
      code: "deepseek-v4-flash",
      name: "DeepSeek V4 Flash",
      modelName: "deepseek-v4-flash",
      modelKind: "llm",
      fallbackPolicy: {},
      status: 1
    }
  ],
  routes: [
    {
      id: 1,
      code: "runtime.llm.chat",
      routePurpose: "chat",
      modelProfileId: 1,
      priority: 100,
      fallbackRouteId: null,
      status: 1,
      policyStatus: {
        networkZone: "public",
        fallbackNetworkZone: null,
        fallbackEnabled: false,
        crossZoneFallbackAllowed: false,
        maxRetries: 0,
        circuitBreakerSeconds: 0,
        violations: []
      },
      maskedCredential: "configured"
    }
  ]
};

describe("AiModelsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getModelRuntimeConfigMock.mockResolvedValue(runtimeSummary);
    getModelRegistryMock.mockResolvedValue(registrySummary);
    upsertModelRegistryRouteMock.mockResolvedValue(registrySummary);
    deleteModelRegistryRouteMock.mockResolvedValue(registrySummary);
    runModelHealthCheckMock.mockResolvedValue({ results: [] });
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  it("loads database registry routes and submits a new model route bundle", async () => {
    render(<AiModelsPage />);

    expect((await screen.findAllByText("runtime.llm.chat")).length).toBeGreaterThan(0);
    expect(screen.getByText("1 Providers")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "新增模型" }));
    fireEvent.change(screen.getByLabelText("模型名称"), {
      target: { value: "deepseek-v4-flash" }
    });
    fireEvent.change(screen.getByLabelText("环境变量引用"), {
      target: { value: "env:LLM_API_KEY" }
    });
    fireEvent.click(screen.getByRole("button", { name: "保存模型" }));

    await waitFor(() =>
      expect(upsertModelRegistryRouteMock).toHaveBeenCalledWith(
        expect.objectContaining({
          credentialRef: "env:LLM_API_KEY",
          modelName: "deepseek-v4-flash",
          routeCode: "runtime.llm.chat",
          routePurpose: "chat"
        })
      )
    );
    await waitFor(() => expect(getModelRegistryMock).toHaveBeenCalledTimes(2));
  });

  it("edits and deletes a database model route from the compact registry table", async () => {
    render(<AiModelsPage />);

    expect((await screen.findAllByText("runtime.llm.chat")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("deepseek-v4-flash").length).toBeGreaterThan(0);

    fireEvent.click(screen.getAllByRole("button", { name: "编辑 runtime.llm.chat" })[0]);
    fireEvent.change(screen.getByLabelText("模型名称"), {
      target: { value: "deepseek-v4-pro" }
    });
    fireEvent.click(screen.getByRole("button", { name: "保存模型" }));

    await waitFor(() =>
      expect(upsertModelRegistryRouteMock).toHaveBeenCalledWith(
        expect.objectContaining({
          credentialRef: null,
          deploymentCode: "deepseek-public",
          modelName: "deepseek-v4-pro",
          routeCode: "runtime.llm.chat"
        })
      )
    );

    fireEvent.click(screen.getAllByRole("button", { name: "删除 runtime.llm.chat" })[0]);

    await waitFor(() => expect(deleteModelRegistryRouteMock).toHaveBeenCalledWith(1));
  });

  it("does not force the registry table wider than the page", async () => {
    render(<AiModelsPage />);

    expect((await screen.findAllByText("runtime.llm.chat")).length).toBeGreaterThan(0);

    const table = screen.getByRole("table");
    expect(table.className).not.toContain("min-w-[980px]");
    expect(table.className).toContain("min-w-full");
  });
});
