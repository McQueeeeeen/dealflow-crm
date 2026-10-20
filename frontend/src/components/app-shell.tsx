"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useState, type ReactNode } from "react";

import { ThemeToggle } from "@/components/theme-toggle";
import { Button, ErrorState, Loading, cx } from "@/components/ui";
import { ROLE_LABELS } from "@/lib/format";
import { useMe } from "@/lib/queries";
import type { Me } from "@/lib/types";

interface NavItem {
  href: string;
  label: string;
  visible?: (me: Me) => boolean;
}

const NAV: NavItem[] = [
  { href: "/today", label: "Today" },
  { href: "/pipeline", label: "Pipeline" },
  { href: "/companies", label: "Companies" },
  { href: "/contacts", label: "Contacts" },
  { href: "/reports", label: "Reports" },
  { href: "/settings", label: "Settings", visible: (me) => me.role === "admin" },
];

function isActive(pathname: string, href: string): boolean {
  if (href === "/pipeline") return pathname === "/pipeline" || pathname.startsWith("/deals/");
  return pathname === href || pathname.startsWith(`${href}/`);
}

export function AppShell({ children }: { children: ReactNode }) {
  const me = useMe();
  const pathname = usePathname();

  if (me.isPending) return <Loading label="Opening your workspace…" />;
  if (me.isError) return <ErrorState error={me.error} onRetry={() => me.refetch()} />;

  const items = NAV.filter((item) => !item.visible || item.visible(me.data));

  return (
    <div className="min-h-dvh">
      <a
        href="#main"
        className="sr-only rounded-control bg-surface px-3 py-2 focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50"
      >
        Skip to content
      </a>
      <header className="sticky top-0 z-30 border-b border-line bg-surface">
        <div className="mx-auto flex max-w-[1600px] items-center gap-4 px-4 sm:px-6">
          <Link href="/today" className="flex min-h-14 items-baseline gap-2 py-2">
            <span className="text-lead font-semibold tracking-tight">DealFlow</span>
            <span className="hidden max-w-48 truncate text-meta text-muted md:inline">
              {me.data.organization_name}
            </span>
          </Link>
          <nav aria-label="Main" className="hidden flex-1 sm:block">
            <NavLinks items={items} pathname={pathname} />
          </nav>
          <div className="ml-auto flex items-center gap-1 sm:ml-0">
            <ThemeToggle />
            <AccountMenu me={me.data} />
          </div>
        </div>
        <nav aria-label="Main" className="overflow-x-auto border-t border-line px-2 sm:hidden">
          <NavLinks items={items} pathname={pathname} />
        </nav>
      </header>
      <main id="main" className="mx-auto max-w-[1600px] px-4 py-6 sm:px-6 sm:py-8">
        {children}
      </main>
    </div>
  );
}

function NavLinks({ items, pathname }: { items: NavItem[]; pathname: string }) {
  return (
    <ul className="flex gap-1">
      {items.map((item) => {
        const active = isActive(pathname, item.href);
        return (
          <li key={item.href}>
            <Link
              href={item.href}
              aria-current={active ? "page" : undefined}
              className={cx(
                // The underline sits on the header's bottom edge so the current page reads as
                // a tab of the workspace.
                "relative inline-flex min-h-12 items-center px-3 text-body whitespace-nowrap transition-colors sm:min-h-14",
                active
                  ? "font-medium text-ink after:absolute after:inset-x-3 after:bottom-0 after:h-0.5 after:bg-accent"
                  : "text-muted hover:text-ink",
              )}
            >
              {item.label}
            </Link>
          </li>
        );
      })}
    </ul>
  );
}

function AccountMenu({ me }: { me: Me }) {
  const [open, setOpen] = useState(false);
  const [signingOut, setSigningOut] = useState(false);

  async function signOut() {
    setSigningOut(true);
    await fetch("/api/auth/logout", { method: "POST" }).catch(() => null);
    window.location.assign("/login");
  }

  return (
    <div
      className="relative"
      onKeyDown={(e) => {
        if (e.key === "Escape") setOpen(false);
      }}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setOpen(false);
      }}
    >
      <button
        type="button"
        aria-expanded={open}
        aria-controls="account-menu"
        onClick={() => setOpen((v) => !v)}
        className="inline-flex min-h-11 items-center gap-2 rounded-control px-2 text-body hover:bg-hover sm:min-h-9"
      >
        <span
          aria-hidden="true"
          className="inline-flex size-7 items-center justify-center rounded-full bg-accent-soft text-meta font-semibold text-accent"
        >
          {initials(me.full_name)}
        </span>
        <span className="hidden max-w-40 truncate lg:inline">{me.full_name}</span>
        <span className="sr-only">Account</span>
      </button>
      {open ? (
        <div
          id="account-menu"
          className="absolute right-0 z-40 mt-2 w-64 rounded-control border border-line bg-raised p-3 shadow-float"
        >
          <p className="font-medium">{me.full_name}</p>
          <p className="truncate text-meta text-muted">{me.email}</p>
          <p className="mt-1 text-meta text-muted">
            {ROLE_LABELS[me.role]} at {me.organization_name}
          </p>
          <Button
            type="button"
            className="mt-3 w-full"
            onClick={signOut}
            busy={signingOut}
          >
            Sign out
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("");
}
