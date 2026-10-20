import type { FieldError } from "./types";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    readonly fields: FieldError[] = [],
  ) {
    super(message);
  }

  fieldError(field: string): string | undefined {
    return this.fields.find((f) => f.field === field)?.message;
  }
}

let refreshing: Promise<boolean> | null = null;

function refreshSession(): Promise<boolean> {
  refreshing ??= fetch("/api/auth/refresh", { method: "POST" })
    .then((res) => res.ok)
    .catch(() => false)
    .finally(() => {
      refreshing = null;
    });
  return refreshing;
}

function redirectToLogin() {
  const next = encodeURIComponent(window.location.pathname + window.location.search);
  window.location.assign(`/login?next=${next}`);
}

async function send(path: string, init: RequestInit): Promise<Response> {
  return fetch(`/api/v1${path}`, {
    ...init,
    headers: init.body ? { "Content-Type": "application/json" } : undefined,
  });
}

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  let res = await send(path, init);
  if (res.status === 401) {
    await refreshSession();
    res = await send(path, init);
    if (res.status === 401) {
      redirectToLogin();
      throw new ApiError(401, "unauthorized", "Your session has ended. Sign in again.");
    }
  }
  if (res.status === 204) {
    return undefined as T;
  }
  const body = await res.json().catch(() => null);
  if (!res.ok) {
    const error = body?.error;
    throw new ApiError(
      res.status,
      error?.code ?? "unknown",
      error?.message ?? `Request failed with status ${res.status}`,
      error?.fields ?? [],
    );
  }
  return body as T;
}

export const get = <T>(path: string) => api<T>(path);
export const post = <T>(path: string, body?: unknown) =>
  api<T>(path, { method: "POST", body: JSON.stringify(body ?? {}) });
export const put = <T>(path: string, body: unknown) =>
  api<T>(path, { method: "PUT", body: JSON.stringify(body) });
export const del = (path: string) => api<void>(path, { method: "DELETE" });

export function query(params: Record<string, string | number | null | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== null && value !== undefined && value !== "") search.set(key, String(value));
  }
  const text = search.toString();
  return text ? `?${text}` : "";
}
