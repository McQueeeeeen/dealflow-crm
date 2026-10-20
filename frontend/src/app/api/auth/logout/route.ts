import { NextResponse, type NextRequest } from "next/server";

import { REFRESH_COOKIE, clearSession, forwardJson } from "@/lib/server/session";

export async function POST(req: NextRequest) {
  const refreshToken = req.cookies.get(REFRESH_COOKIE)?.value;
  if (refreshToken) {
    // Signing out locally must work even when the API is down.
    await forwardJson("/api/v1/auth/logout", { refresh_token: refreshToken }).catch(() => null);
  }
  return clearSession(new NextResponse(null, { status: 204 }));
}
