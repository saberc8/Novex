import type { ApiResponse } from "@/types/api";
import { getAuthToken } from "./auth";

const DEFAULT_API_BASE_URL = "http://localhost:4398";

export class ApiClientError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ApiClientError";
  }
}

export interface RequestOptions {
  method?: "GET" | "POST";
  body?: unknown;
  query?: object;
}

export async function apiRequest<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const url = apiUrl(path, options.query);
  const token = getAuthToken();
  const headers: Record<string, string> = {};

  if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const response = await fetch(url, {
    method: options.method ?? "GET",
    headers,
    body: options.body === undefined ? undefined : JSON.stringify(options.body)
  });

  if (!response.ok) {
    throw new ApiClientError(`请求失败：HTTP ${response.status}`);
  }

  const envelope = (await response.json()) as ApiResponse<T>;
  if (!envelope.success || envelope.code !== "200") {
    throw new ApiClientError(envelope.msg || "请求失败");
  }

  return envelope.data;
}

export async function apiFormRequest<T>(path: string, form: FormData): Promise<T> {
  const url = apiUrl(path);
  const token = getAuthToken();
  const headers: Record<string, string> = {};

  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const response = await fetch(url, {
    method: "POST",
    headers,
    body: form
  });

  if (!response.ok) {
    throw new ApiClientError(`请求失败：HTTP ${response.status}`);
  }

  const envelope = (await response.json()) as ApiResponse<T>;
  if (!envelope.success || envelope.code !== "200") {
    throw new ApiClientError(envelope.msg || "请求失败");
  }

  return envelope.data;
}

export function apiUrl(path: string, query?: object) {
  const base = process.env.NEXT_PUBLIC_API_BASE_URL || DEFAULT_API_BASE_URL;
  const url = new URL(path, base);

  Object.entries(query ?? {}).forEach(([key, value]) => {
    appendQuery(url, key, value);
  });

  return url.toString();
}

function appendQuery(url: URL, key: string, value: unknown) {
  if (value === undefined || value === null || value === "") {
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item) => appendQuery(url, key, item));
    return;
  }
  url.searchParams.append(key, String(value));
}
