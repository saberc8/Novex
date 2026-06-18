const TOKEN_KEY = "novex_token";
const DEFAULT_API_BASE_URL = "http://localhost:4398";
const DEFAULT_DEV_USERNAME = "admin";
const DEFAULT_DEV_PASSWORD = "admin123";
const DEFAULT_DEV_CLIENT_ID = "codex-app-poc";

type LoginEnvelope = {
  code?: string;
  data?: {
    token?: string;
  };
  msg?: string;
  message?: string;
};

let pendingDevLogin: Promise<string | null> | null = null;

export function getAuthToken() {
  if (typeof window === "undefined" || !window.localStorage) {
    return null;
  }
  return window.localStorage.getItem(TOKEN_KEY);
}

export function setAuthToken(token: string) {
  if (typeof window === "undefined" || !window.localStorage) {
    return;
  }
  window.localStorage.setItem(TOKEN_KEY, token);
}

export function clearAuthToken() {
  if (typeof window === "undefined" || !window.localStorage) {
    return;
  }
  window.localStorage.removeItem(TOKEN_KEY);
}

export async function ensureDevAuthToken() {
  const existing = getAuthToken();
  if (existing) {
    return existing;
  }
  if (!isDevAutoLoginEnabled()) {
    return null;
  }

  pendingDevLogin ??= requestDevAuthToken().finally(() => {
    pendingDevLogin = null;
  });
  return pendingDevLogin;
}

function isDevAutoLoginEnabled() {
  return (process.env.NEXT_PUBLIC_DEV_AUTO_LOGIN ?? "1").trim() !== "0";
}

async function requestDevAuthToken() {
  const username = process.env.NEXT_PUBLIC_DEV_LOGIN_USERNAME?.trim() || DEFAULT_DEV_USERNAME;
  const password = process.env.NEXT_PUBLIC_DEV_LOGIN_PASSWORD?.trim() || DEFAULT_DEV_PASSWORD;
  const clientId = process.env.NEXT_PUBLIC_DEV_CLIENT_ID?.trim() || DEFAULT_DEV_CLIENT_ID;
  const response = await fetch(apiUrl("/auth/login"), {
    method: "POST",
    headers: {
      "Content-Type": "application/json"
    },
    body: JSON.stringify({
      username,
      password,
      authType: "ACCOUNT",
      clientId
    })
  });
  const body = (await response.json()) as LoginEnvelope;
  const token = body.data?.token?.trim();
  if (!response.ok || body.code !== "200" || !token) {
    throw new Error(body.msg ?? body.message ?? "本地开发登录失败");
  }

  setAuthToken(token);
  return token;
}

function apiUrl(path: string) {
  return new URL(path, apiBaseUrl()).toString();
}

function apiBaseUrl() {
  return (process.env.NEXT_PUBLIC_API_BASE_URL ?? DEFAULT_API_BASE_URL).replace(/\/$/, "");
}
