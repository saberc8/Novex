import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  generateCustomerPackage,
  getDeliveryTemplate,
  listDeliveryTemplates
} from "@/api/ai/template";

function okResponse(data: unknown = true) {
  return Promise.resolve(
    new Response(
      JSON.stringify({
        code: "200",
        data,
        msg: "成功",
        success: true,
        timestamp: "1"
      }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" }
      }
    )
  );
}

describe("delivery template api wrappers", () => {
  const fetchMock = vi.fn<typeof fetch>(() => okResponse());

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("uses template list, detail, and customer package endpoints", async () => {
    await listDeliveryTemplates({ page: 1, size: 20, category: "training" });
    await getDeliveryTemplate("training_app");
    await generateCustomerPackage({
      templateCode: "training_app",
      customerName: "Acme",
      appName: "Acme Training",
      industry: "training",
      brandName: "Acme Academy",
      primaryColor: "#2563eb",
      publicUrl: "https://training.example.com"
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://localhost:4398/ai/templates?page=1&size=20&category=training"
    );
    expect(fetchMock.mock.calls[1]?.[0]).toBe("http://localhost:4398/ai/templates/training_app");
    expect(fetchMock.mock.calls[2]?.[0]).toBe("http://localhost:4398/ai/templates/packages");
    expect(fetchMock.mock.calls[2]?.[1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({
        templateCode: "training_app",
        customerName: "Acme",
        appName: "Acme Training",
        industry: "training",
        brandName: "Acme Academy",
        primaryColor: "#2563eb",
        publicUrl: "https://training.example.com"
      })
    });
  });
});
