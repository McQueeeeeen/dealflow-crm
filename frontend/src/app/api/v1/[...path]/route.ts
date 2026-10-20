import { NextResponse, type NextRequest } from "next/server";

import { ACCESS_COOKIE, apiUrl, unavailable } from "@/lib/server/session";

type Context = { params: Promise<{ path: string[] }> };

async function proxy(req: NextRequest, { params }: Context): Promise<Response> {
  const token = req.cookies.get(ACCESS_COOKIE)?.value;
  if (!token) {
    return NextResponse.json(
      { error: { code: "unauthorized", message: "authentication required" } },
      { status: 401 },
    );
  }

  const { path } = await params;
  if (path.some((segment) => segment === "." || segment === "..")) {
    return NextResponse.json({ error: { code: "not_found" } }, { status: 404 });
  }
  const target = apiUrl(`/api/v1/${path.map(encodeURIComponent).join("/")}${req.nextUrl.search}`);
  const hasBody = req.method !== "GET" && req.method !== "HEAD";
  const upstream = await fetch(target, {
    method: req.method,
    headers: {
      Authorization: `Bearer ${token}`,
      ...(hasBody ? { "Content-Type": "application/json" } : {}),
    },
    body: hasBody ? await req.text() : undefined,
    cache: "no-store",
    signal: req.signal,
  }).catch(() => null);
  if (!upstream) return unavailable();

  const headers = new Headers();
  for (const name of ["Content-Type", "Content-Disposition", "Cache-Control"]) {
    const value = upstream.headers.get(name);
    if (value) headers.set(name, value);
  }
  return new Response(upstream.status === 204 ? null : upstream.body, {
    status: upstream.status,
    headers,
  });
}

export const maxDuration = 300;

export { proxy as GET, proxy as POST, proxy as PUT, proxy as DELETE };
