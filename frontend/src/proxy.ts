import { NextResponse, type NextRequest } from "next/server";

const REFRESH_COOKIE = "df_refresh";
const PUBLIC_PAGES = ["/login", "/register"];

// Routing only: a missing session cookie sends people to sign in. The API still checks every
// request, so this is a convenience, not the security boundary.
export function proxy(req: NextRequest) {
  const signedIn = req.cookies.has(REFRESH_COOKIE);
  const { pathname, search } = req.nextUrl;
  const isPublic = PUBLIC_PAGES.includes(pathname);

  if (!signedIn && !isPublic) {
    const url = req.nextUrl.clone();
    url.pathname = "/login";
    url.search = pathname === "/" ? "" : `?next=${encodeURIComponent(pathname + search)}`;
    return NextResponse.redirect(url);
  }
  if (signedIn && isPublic) {
    const url = req.nextUrl.clone();
    url.pathname = "/today";
    url.search = "";
    return NextResponse.redirect(url);
  }
  return NextResponse.next();
}

export const config = {
  matcher: ["/((?!api|_next/static|_next/image|favicon.ico|icon.svg|manifest.webmanifest).*)"],
};
