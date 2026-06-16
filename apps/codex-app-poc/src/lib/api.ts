const DEFAULT_API_BASE_URL = "http://localhost:4398";

type ApiEnvelope<T> = {
  code?: string;
  data?: T;
  msg?: string;
  message?: string;
};

export async function apiRequest<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (!headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const response = await fetch(`${apiBaseUrl()}${path}`, {
    ...init,
    headers
  });
  const body = (await response.json()) as ApiEnvelope<T>;

  if (!response.ok || body.code !== "200") {
    throw new Error(body.msg ?? body.message ?? "Request failed");
  }

  return body.data as T;
}

function apiBaseUrl() {
  return (process.env.NEXT_PUBLIC_API_BASE_URL ?? DEFAULT_API_BASE_URL).replace(/\/$/, "");
}
