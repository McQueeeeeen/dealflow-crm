import "server-only";

import { NextResponse } from "next/server";

// Tokens never reach browser JavaScript: the Next.js server keeps them in httpOnly cookies
// and attaches the access token when it forwards requests to the API.
export const ACCESS_COOKIE = "df_access";
export const REFRESH_COOKIE = "df_refresh";
const REFRESH_MAX_AGE = 60 * 60 * 24 * 30;

export function apiUrl(path: string): string {
  const base = process.env.API_URL ?? "http://localhost:8080";
  return `${base.replace(/\/$/, "")}${path}`;
}

interface SessionBody {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: unknown;
}

const secure = process.env.NODE_ENV === "production";

export function withSession(body: SessionBody, status = 200): NextResponse {
  const res = NextResponse.json({ user: body.user }, { status });
  res.cookies.set(ACCESS_COOKIE, body.access_token, {
    httpOnly: true,
    secure,
    sameSite: "lax",
    path: "/",
    maxAge: body.expires_in,
  });
  res.cookies.set(REFRESH_COOKIE, body.refresh_token, {
    httpOnly: true,
    secure,
    sameSite: "lax",
    path: "/",
    maxAge: REFRESH_MAX_AGE,
  });
  return res;
}

export function clearSession(res: NextResponse): NextResponse {
  res.cookies.delete(ACCESS_COOKIE);
  res.cookies.delete(REFRESH_COOKIE);
  return res;
}

/** Forwards a JSON body to the API and relays its status and payload. */
export async function forwardJson(path: string, body: unknown): Promise<Response> {
  return fetch(apiUrl(path), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
    cache: "no-store",
  });
}

export function unavailable(): NextResponse {
  return NextResponse.json(
    {
      error: {
        code: "api_unavailable",
        message: "The CRM server is not reachable. Try again shortly.",
      },
    },
    { status: 503 },
  );
}
