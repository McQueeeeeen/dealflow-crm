import { NextResponse, type NextRequest } from "next/server";

import {
  REFRESH_COOKIE,
  clearSession,
  forwardJson,
  unavailable,
  withSession,
} from "@/lib/server/session";

export async function POST(req: NextRequest) {
  const refreshToken = req.cookies.get(REFRESH_COOKIE)?.value;
  if (!refreshToken) {
    return NextResponse.json({ error: { code: "unauthorized" } }, { status: 401 });
  }
  const res = await forwardJson("/api/v1/auth/refresh", { refresh_token: refreshToken }).catch(
    () => null,
  );
  if (!res) return unavailable();
  if (!res.ok) {
    return clearSession(NextResponse.json({ error: { code: "unauthorized" } }, { status: 401 }));
  }
  return withSession(await res.json());
}
