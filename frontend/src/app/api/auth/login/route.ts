import { NextResponse, type NextRequest } from "next/server";

import { forwardJson, unavailable, withSession } from "@/lib/server/session";

export async function POST(req: NextRequest) {
  const body = await req.json().catch(() => ({}));
  const res = await forwardJson("/api/v1/auth/login", body).catch(() => null);
  if (!res) return unavailable();
  const payload = await res.json();
  return res.ok ? withSession(payload) : NextResponse.json(payload, { status: res.status });
}
