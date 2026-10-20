"use client";

import {
  IconBuilding,
  IconCalendarCheck,
  IconChartBar,
  IconLayoutKanban,
  IconPlus,
  IconSearch,
  IconSunMoon,
  IconUser,
  IconUsers,
  type Icon,
} from "@tabler/icons-react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useRouter } from "next/navigation";
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";

import { DealFormDialog } from "@/components/deal-form";
import { toggleTheme } from "@/components/theme-toggle";
import { cx } from "@/components/ui";
import { get, query } from "@/lib/api";
import { useDebounced } from "@/lib/hooks";
import type { Hit, SearchResults } from "@/lib/types";

const OPEN_EVENT = "dealflow-palette";

interface Item {
  id: string;
  group: string;
  label: string;
  detail?: string | null;
  icon: Icon;
  run: () => void;
}

export function openPalette() {
  window.dispatchEvent(new Event(OPEN_EVENT));
}

export function SearchButton({ compact }: { compact?: boolean }) {
  return (
    <button
      type="button"
      onClick={openPalette}
      aria-keyshortcuts="Meta+K Control+K"
      className={cx(
        "text-body text-muted hover:bg-hover hover:text-ink flex min-h-10 items-center gap-3 rounded-lg px-3",
        !compact && "border-line bg-row shadow-highlight w-full border",
      )}
    >
      <IconSearch size={18} stroke={1.75} aria-hidden="true" />
      <span className={cx(compact && "sr-only")}>Search</span>
      {compact ? null : (
        <kbd className="border-line text-meta ml-auto rounded-md border px-1.5 font-sans">⌘K</kbd>
      )}
    </button>
  );
}

export function CommandPalette() {
  const router = useRouter();
  const dialog = useRef<HTMLDialogElement>(null);
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [active, setActive] = useState(0);
  const [creating, setCreating] = useState(false);
  const q = useDebounced(text.trim(), 150);
  const listId = useId();

  const results = useQuery({
    queryKey: ["search", q],
    queryFn: () => get<SearchResults>(`/search${query({ q })}`),
    enabled: open && q.length >= 2,
    placeholderData: keepPreviousData,
  });

  useEffect(() => {
    const show = () => setOpen(true);
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    window.addEventListener(OPEN_EVENT, show);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener(OPEN_EVENT, show);
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  useEffect(() => {
    const d = dialog.current;
    if (open && d && !d.open) d.showModal();
    if (!open && d?.open) d.close();
  }, [open]);

  const go = (href: string) => () => router.push(href);
  const hits = (list: Hit[] | undefined, group: string, icon: Icon, base: string): Item[] =>
    (list ?? []).map((h) => ({
      id: `${base}-${h.id}`,
      group,
      label: h.title,
      detail: h.detail,
      icon,
      run: go(`/${base}/${h.id}`),
    }));

  const needle = text.trim().toLowerCase();
  const found = q.length >= 2 ? results.data : undefined;
  const commands: Item[] = [
    { id: "today", group: "Go to", label: "Today", icon: IconCalendarCheck, run: go("/today") },
    {
      id: "pipeline",
      group: "Go to",
      label: "Pipeline",
      icon: IconLayoutKanban,
      run: go("/pipeline"),
    },
    {
      id: "companies",
      group: "Go to",
      label: "Companies",
      icon: IconBuilding,
      run: go("/companies"),
    },
    { id: "contacts", group: "Go to", label: "Contacts", icon: IconUsers, run: go("/contacts") },
    { id: "reports", group: "Go to", label: "Reports", icon: IconChartBar, run: go("/reports") },
    {
      id: "new-deal",
      group: "Actions",
      label: "New deal",
      icon: IconPlus,
      run: () => setCreating(true),
    },
    { id: "theme", group: "Actions", label: "Switch theme", icon: IconSunMoon, run: toggleTheme },
  ];
  const items = [
    ...hits(found?.deals, "Deals", IconLayoutKanban, "deals"),
    ...hits(found?.companies, "Companies", IconBuilding, "companies"),
    ...hits(found?.contacts, "People", IconUser, "contacts"),
    ...commands.filter((c) => !needle || c.label.toLowerCase().includes(needle)),
  ];
  const current = Math.min(active, items.length - 1);
  const groups = [...new Set(items.map((i) => i.group))];

  function choose(item: Item) {
    setOpen(false);
    item.run();
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if ((e.key === "ArrowDown" || e.key === "ArrowUp") && items.length > 0) {
      e.preventDefault();
      const by = e.key === "ArrowDown" ? 1 : -1;
      setActive((current + by + items.length) % items.length);
    } else if (e.key === "Enter" && items[current]) {
      e.preventDefault();
      choose(items[current]);
    }
  }

  return (
    <>
      <dialog
        ref={dialog}
        aria-label="Search and commands"
        onClose={() => {
          setOpen(false);
          setText("");
          setActive(0);
        }}
        className="rounded-dialog border-line bg-raised text-ink shadow-float mx-auto mt-[12vh] w-[calc(100%-2rem)] max-w-xl border p-0"
      >
        {open ? (
          <>
            <div className="border-line flex items-center gap-3 border-b px-4">
              <IconSearch size={20} stroke={1.75} className="text-muted" aria-hidden="true" />
              <input
                autoFocus
                role="combobox"
                aria-label="Search deals, companies and people"
                aria-expanded="true"
                aria-controls={listId}
                aria-autocomplete="list"
                aria-activedescendant={items[current] ? `${listId}-${items[current].id}` : undefined}
                value={text}
                onChange={(e) => {
                  setText(e.target.value);
                  setActive(0);
                }}
                onKeyDown={onKeyDown}
                placeholder="Search deals, companies, people…"
                autoComplete="off"
                spellCheck={false}
                className="text-lead placeholder:text-muted h-14 min-w-0 flex-1 bg-transparent focus-visible:outline-none"
              />
              <kbd className="border-line text-meta text-muted rounded-md border px-1.5">Esc</kbd>
            </div>
            <div id={listId} role="listbox" className="max-h-[60vh] overflow-y-auto p-2">
              {groups.map((group) => (
                <div key={group} role="group" aria-labelledby={`${listId}-${group}`}>
                  <p
                    id={`${listId}-${group}`}
                    className="text-meta text-muted px-3 pt-3 pb-1 font-medium"
                  >
                    {group}
                  </p>
                  {items
                    .filter((item) => item.group === group)
                    .map((item) => {
                      const index = items.indexOf(item);
                      return (
                        <div
                          key={item.id}
                          id={`${listId}-${item.id}`}
                          role="option"
                          aria-selected={index === current}
                          onMouseMove={() => setActive(index)}
                          onClick={() => choose(item)}
                          className={cx(
                            "flex min-h-11 cursor-pointer items-center gap-3 rounded-lg px-3",
                            index === current && "bg-hover",
                          )}
                        >
                          <item.icon
                            size={18}
                            stroke={1.75}
                            className="text-muted shrink-0"
                            aria-hidden="true"
                          />
                          <span className="min-w-0 flex-1 truncate">{item.label}</span>
                          {item.detail ? (
                            <span className="text-meta text-muted max-w-[45%] truncate">
                              {item.detail}
                            </span>
                          ) : null}
                        </div>
                      );
                    })}
                </div>
              ))}
              {items.length === 0 ? (
                <p className="text-body text-muted px-3 py-8 text-center">
                  {results.isFetching ? "Searching…" : `Nothing matches “${text.trim()}”.`}
                </p>
              ) : null}
            </div>
          </>
        ) : null}
      </dialog>
      <DealFormDialog
        open={creating}
        onClose={() => setCreating(false)}
        onSaved={(deal) => router.push(`/deals/${deal.id}`)}
      />
    </>
  );
}
