import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { accountLogin, getImageCaptcha } from "./auth";

function okResponse(data: unknown) {
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

describe("training auth api", () => {
  const fetchMock = vi.fn<typeof fetch>(() =>
    okResponse({
      isEnabled: false,
      uuid: "",
      img: ""
    })
  );

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fetchMock.mockClear();
  });

  it("loads image captcha before account login", async () => {
    await getImageCaptcha();

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:62601/captcha/image");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "GET"
    });
  });

  it("submits account login with the training web client id", async () => {
    fetchMock.mockImplementationOnce(() =>
      okResponse({
        token: "token-1",
        expire: "2099-01-01T00:00:00Z"
      })
    );

    await accountLogin({
      username: "employee",
      password: "employee123",
      authType: "ACCOUNT",
      clientId: "novex-training-web"
    });

    expect(fetchMock.mock.calls[0]?.[0]).toBe("http://localhost:62601/auth/login");
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: expect.objectContaining({
        "Content-Type": "application/json"
      }),
      body: JSON.stringify({
        username: "employee",
        password: "employee123",
        authType: "ACCOUNT",
        clientId: "novex-training-web"
      })
    });
  });
});
