import { getAuthToken } from "./auth";

const DEFAULT_API_BASE_URL = "http://localhost:62601";

type ApiEnvelope<T> = {
  code?: string;
  data?: T;
  msg?: string;
  message?: string;
};

type ApiRequestInit = RequestInit & {
  query?: Record<string, unknown>;
};

export async function apiRequest<T>(path: string, init: ApiRequestInit = {}): Promise<T> {
  const { query, ...requestInit } = init;
  const headers: Record<string, string> = {};
  new Headers(requestInit.headers).forEach((value, key) => {
    headers[key] = value;
  });
  if (!("Content-Type" in headers) && !("content-type" in headers)) {
    headers["Content-Type"] = "application/json";
  }
  const token = getAuthToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const response = await fetch(apiUrl(path, query), {
    ...requestInit,
    headers
  });
  const body = (await response.json()) as ApiEnvelope<T>;

  if (!response.ok || body.code !== "200") {
    throw new Error(body.msg ?? body.message ?? "Request failed");
  }

  return body.data as T;
}

export function apiUrl(path: string, query?: Record<string, unknown>) {
  const url = new URL(path, apiBaseUrl());
  Object.entries(query ?? {}).forEach(([key, value]) => {
    if (value === undefined || value === null || value === "") {
      return;
    }
    url.searchParams.append(key, String(value));
  });
  return url.toString();
}

function apiBaseUrl() {
  return (process.env.NEXT_PUBLIC_API_BASE_URL ?? DEFAULT_API_BASE_URL).replace(/\/$/, "");
}
