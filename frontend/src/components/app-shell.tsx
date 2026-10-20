"use client";

import {
  IconBuilding,
  IconCalendarCheck,
  IconChartBar,
  IconLayoutKanban,
  IconSettings,
  IconUsers,
  type Icon,
} from "@tabler/icons-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useState, type ReactNode } from "react";

import { CommandPalette, SearchButton } from "@/components/command-palette";
import { LiveProvider, LiveStatus } from "@/components/live";
import { ThemeToggle } from "@/components/theme-toggle";
import { Avatar, Button, ErrorState, Loading, cx } from "@/components/ui";
import { ROLE_LABELS } from "@/lib/format";
import { dueToday, useMe, useMyTasks } from "@/lib/queries";
import { SECTION } from "@/lib/transitions";
import type { Me } from "@/lib/types";

interface NavItem {
  href: string;
  label: string;
  icon: Icon;
  visible?: (me: Me) => boolean;
}

const NAV: NavItem[] = [
  { href: "/today", label: "Today", icon: IconCalendarCheck },
  { href: "/pipeline", label: "Pipeline", icon: IconLayoutKanban },
  { href: "/companies", label: "Companies", icon: IconBuilding },
  { href: "/contacts", label: "Contacts", icon: IconUsers },
  { href: "/reports", label: "Reports", icon: IconChartBar },
  {
    href: "/settings",
    label: "Settings",
    icon: IconSettings,
    visible: (me) => me.role === "admin",
  },
];

function isActive(pathname: string, href: string): boolean {
  if (href === "/pipeline") return pathname === "/pipeline" || pathname.startsWith("/deals/");
  return pathname === href || pathname.startsWith(`${href}/`);
}

export function AppShell({ children }: { children: ReactNode }) {
  const me = useMe();
  const pathname = usePathname();
  const tasks = useMyTasks().data?.data;
  const due = tasks ? dueToday(tasks) : 0;

  if (me.isPending) return <Loading label="Opening your workspace…" />;
  if (me.isError) return <ErrorState error={me.error} onRetry={() => me.refetch()} />;

  const items = NAV.filter((item) => !item.visible || item.visible(me.data));
  const section = items.find((item) => isActive(pathname, item.href));

  return (
    <LiveProvider me={me.data.id}>
    <div className="min-h-dvh lg:grid lg:grid-cols-[15rem_minmax(0,1fr)]">
      <a
        href="#main"
        className="rounded-control bg-surface sr-only px-3 py-2 focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50"
      >
        Skip to content
      </a>

      <div className="border-line bg-sidebar hidden border-r lg:block">
        <aside
          className="sticky top-0 flex h-dvh flex-col px-3 py-4"
          style={{ viewTransitionName: "app-header" }}
        >
          <Brand me={me.data} />
          <div className="mt-3">
            <SearchButton />
          </div>
          <nav aria-label="Main" className="border-line mt-3 border-t pt-3">
            <NavLinks items={items} pathname={pathname} due={due} vertical />
          </nav>
          <div className="border-line mt-auto flex flex-col gap-1 border-t pt-3">
            <ThemeToggle />
            <AccountMenu me={me.data} up />
          </div>
        </aside>
      </div>

      <div className="min-w-0">
        <header
          className="border-line bg-sidebar sticky top-0 z-30 border-b lg:hidden"
          style={{ viewTransitionName: "app-header" }}
        >
          <div className="flex items-center gap-2 px-4">
            <Brand me={me.data} />
            <div className="ml-auto flex items-center gap-1">
              <SearchButton compact />
              <ThemeToggle compact />
              <AccountMenu me={me.data} />
            </div>
          </div>
          <nav aria-label="Main" className="border-line overflow-x-auto border-t px-2">
            <NavLinks items={items} pathname={pathname} due={due} />
          </nav>
        </header>

        {section ? (
          <div className="border-line text-muted hidden h-14 items-center gap-2 border-b px-8 lg:flex">
            <section.icon size={18} stroke={1.75} aria-hidden="true" />
            <span>{section.label}</span>
            <LiveStatus me={me.data.id} />
          </div>
        ) : null}

        <main id="main" className="max-w-[1500px] px-4 py-6 sm:px-6 lg:px-8 lg:py-8">
          {children}
        </main>
      </div>
      <CommandPalette />
    </div>
    </LiveProvider>
  );
}

function Brand({ me }: { me: Me }) {
  return (
    <Link
      transitionTypes={SECTION}
      href="/today"
      className="rounded-control flex min-h-12 min-w-0 items-center gap-3 px-2"
    >
      <span
        aria-hidden="true"
        className="bg-accent shadow-highlight flex size-9 shrink-0 items-end justify-center gap-[3px] rounded-xl pb-2"
      >
        {[8, 13, 18].map((h) => (
          <span key={h} className="w-[3px] rounded-full bg-white" style={{ height: h }} />
        ))}
      </span>
      <span className="flex min-w-0 flex-col">
        <span className="text-lead font-display" translate="no">
          DealFlow
        </span>
        <span className="text-meta text-muted truncate">{me.organization_name}</span>
      </span>
    </Link>
  );
}

function NavLinks({
  items,
  pathname,
  due,
  vertical,
}: {
  items: NavItem[];
  pathname: string;
  due: number;
  vertical?: boolean;
}) {
  return (
    <ul className={cx("flex gap-1", vertical && "flex-col")}>
      {items.map((item) => {
        const active = isActive(pathname, item.href);
        return (
          <li key={item.href}>
            <Link
              transitionTypes={SECTION}
              href={item.href}
              aria-current={active ? "page" : undefined}
              className={cx(
                "text-body flex min-h-10 items-center gap-3 rounded-lg border px-3 font-medium whitespace-nowrap transition-colors",
                !vertical && "my-2",
                active
                  ? "border-line bg-hover text-ink shadow-highlight"
                  : "text-muted hover:bg-hover hover:text-ink border-transparent",
              )}
            >
              <item.icon size={18} stroke={1.75} aria-hidden="true" />
              {item.label}
              {item.href === "/today" && due > 0 ? (
                <span className="num bg-accent text-accent-ink ml-auto inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1.5 text-[0.6875rem] font-semibold">
                  {due}
                  <span className="sr-only"> due today</span>
                </span>
              ) : null}
            </Link>
          </li>
        );
      })}
    </ul>
  );
}

function AccountMenu({ me, up }: { me: Me; up?: boolean }) {
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
        className={cx(
          "text-body hover:bg-hover flex min-h-11 items-center gap-2.5 rounded-lg px-2 text-left",
          up && "w-full",
        )}
      >
        <Avatar name={me.full_name} large />
        <span className={cx("min-w-0", !up && "sr-only")}>
          <span className="block truncate font-medium">{me.full_name}</span>
          <span className="text-meta text-muted block truncate">{ROLE_LABELS[me.role]}</span>
        </span>
        <span className="sr-only">Account</span>
      </button>
      {open ? (
        <div
          id="account-menu"
          className={cx(
            "rounded-card border-line bg-raised shadow-float absolute z-40 w-64 border p-3",
            up ? "bottom-full left-0 mb-2" : "top-full right-0 mt-2",
          )}
        >
          <p className="font-medium">{me.full_name}</p>
          <p className="text-meta text-muted truncate">{me.email}</p>
          <p className="text-meta text-muted mt-1">
            {ROLE_LABELS[me.role]} at {me.organization_name}
          </p>
          <Button type="button" className="mt-3 w-full" onClick={signOut} busy={signingOut}>
            Sign Out
          </Button>
        </div>
      ) : null}
    </div>
  );
}
