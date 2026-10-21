"use client";

import { IconFilter, IconHelp, IconX } from "@tabler/icons-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useId, useState } from "react";

import { Button, FormError, TextInput, cx } from "@/components/ui";
import { ApiError, del, get, post, query } from "@/lib/api";
import { useDebounced } from "@/lib/hooks";
import { useInvalidate, useMe } from "@/lib/queries";
import type { Deal, Page, SavedView } from "@/lib/types";

const SYNTAX: [string, string][] = [
  ["fleet", "words in the deal name"],
  ["value>50k", "amount, also <, <=, >=, value:12,500"],
  ["stage:prop", "stage name starts with"],
  ["owner:me", "owner, or part of a name"],
  ["company:harbor", "part of the company name"],
  ["status:won", "open, won or lost"],
  ["closes<30d", "expected close: today, 2026-12-31, 2w, 3m"],
  ["quiet>14", "days without activity"],
  ["age<7", "days since the deal was created"],
  ["-status:lost", "leave out matches, or NOT"],
  ["(a OR b)", "either side, grouped"],
];

export function FilterBar({
  filter,
  onApply,
}: {
  filter: string;
  onApply: (filter: string) => void;
}) {
  const me = useMe().data;
  const invalidate = useInvalidate();
  const inputId = useId();
  const [draft, setDraft] = useState(filter);
  const [help, setHelp] = useState(false);
  const [naming, setNaming] = useState(false);
  const text = useDebounced(draft.trim(), 300);

  const check = useQuery({
    queryKey: ["filter-check", text],
    queryFn: () => get<Page<Deal>>(`/deals${query({ filter: text, per_page: 1 })}`),
    enabled: text !== "",
    retry: false,
    staleTime: 30_000,
  });
  const views = useQuery({ queryKey: ["views"], queryFn: () => get<SavedView[]>("/views") });

  useEffect(() => {
    if (text !== filter && (text === "" || check.isSuccess)) onApply(text);
  }, [text, filter, check.isSuccess, onApply]);

  const invalid =
    text !== "" && check.error instanceof ApiError && check.error.code === "invalid_filter"
      ? check.error
      : null;

  return (
    <div className="mb-4 flex min-w-0 flex-col gap-2">
      <div className="flex items-center gap-2">
        <label htmlFor={inputId} className="sr-only">
          Filter deals
        </label>
        <div className="relative min-w-0 flex-1 sm:max-w-xl">
          <IconFilter
            size={16}
            stroke={1.75}
            aria-hidden="true"
            className="text-muted pointer-events-none absolute top-1/2 left-3 -translate-y-1/2"
          />
          <TextInput
            id={inputId}
            type="search"
            name="filter"
            spellCheck={false}
            placeholder="fleet value>50k owner:me closes<30d"
            value={draft}
            aria-invalid={invalid ? true : undefined}
            aria-describedby={`${inputId}-status`}
            onChange={(e) => setDraft(e.target.value)}
            className="pl-9 font-mono"
          />
        </div>
        <Button
          type="button"
          variant="quiet"
          aria-expanded={help}
          aria-controls={`${inputId}-help`}
          onClick={() => setHelp((h) => !h)}
        >
          <IconHelp size={16} stroke={1.75} aria-hidden="true" />
          <span className="sr-only sm:not-sr-only">Syntax</span>
        </Button>
        {text && check.isSuccess && !naming ? (
          <Button type="button" onClick={() => setNaming(true)}>
            Save View
          </Button>
        ) : null}
      </div>

      <p id={`${inputId}-status`} aria-live="polite" className="text-meta min-h-5">
        {invalid ? (
          <span className="text-critical">
            <Marked text={text} span={invalid.span} /> {invalid.message}
          </span>
        ) : text && check.isSuccess ? (
          <span className="text-muted num">
            {check.data.total} {check.data.total === 1 ? "deal matches" : "deals match"}
          </span>
        ) : null}
      </p>

      {help ? (
        <dl
          id={`${inputId}-help`}
          className="border-line bg-surface shadow-highlight grid gap-x-6 gap-y-1 rounded-xl border p-3 sm:grid-cols-2"
        >
          {SYNTAX.map(([example, meaning]) => (
            <div key={example} className="flex items-baseline gap-3">
              <dt>
                <button
                  type="button"
                  onClick={() => setDraft((d) => `${d.trim()} ${example}`.trim())}
                  className="text-link text-body rounded font-mono hover:underline"
                >
                  {example}
                </button>
              </dt>
              <dd className="text-meta text-muted">{meaning}</dd>
            </div>
          ))}
        </dl>
      ) : null}

      {naming ? (
        <SaveView
          filter={text}
          onDone={() => {
            setNaming(false);
            invalidate("view");
          }}
        />
      ) : null}

      {views.data && views.data.length > 0 ? (
        <ul aria-label="Saved views" className="flex flex-wrap gap-2">
          {views.data.map((view) => {
            const active = view.filter === text;
            const removable = view.user_id === me?.id || me?.role === "admin";
            return (
              <li
                key={view.id}
                className={cx(
                  "text-body flex items-center rounded-full border",
                  active ? "border-accent bg-accent-soft" : "border-line bg-row",
                )}
              >
                <button
                  type="button"
                  aria-pressed={active}
                  title={view.filter}
                  onClick={() => setDraft(active ? "" : view.filter)}
                  className="min-h-8 rounded-full px-3 font-medium"
                >
                  {view.name}
                  {view.shared && view.user_id !== me?.id ? (
                    <span className="text-muted font-normal"> by {view.owner_name}</span>
                  ) : null}
                </button>
                {removable ? <RemoveView view={view} /> : null}
              </li>
            );
          })}
        </ul>
      ) : null}
    </div>
  );
}

export function Marked({ text, span }: { text: string; span?: [number, number] }) {
  if (!span) return null;
  const chars = Array.from(text);
  const [start, end] = [span[0], Math.max(span[1], span[0] + 1)];
  return (
    <span className="text-ink font-mono">
      {chars.slice(0, start).join("")}
      <mark className="text-critical decoration-critical bg-transparent underline decoration-wavy underline-offset-4">
        {chars.slice(start, end).join("") || " "}
      </mark>
      {chars.slice(end).join("")}
    </span>
  );
}

function SaveView({ filter, onDone }: { filter: string; onDone: () => void }) {
  const [name, setName] = useState("");
  const [shared, setShared] = useState(false);
  const save = useMutation({
    mutationFn: () => post<SavedView>("/views", { name, filter, shared }),
    onSuccess: onDone,
  });
  return (
    <form
      className="border-line bg-surface flex flex-wrap items-center gap-3 rounded-xl border p-3"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <label className="flex min-w-0 flex-1 items-center gap-2">
        <span className="text-body text-muted shrink-0">Name</span>
        <TextInput
          autoFocus
          name="view-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Big deals closing soon"
          className="min-w-0"
        />
      </label>
      <label className="text-body flex items-center gap-2">
        <input
          type="checkbox"
          checked={shared}
          onChange={(e) => setShared(e.target.checked)}
          className="size-4 accent-[var(--accent)]"
        />
        Share with the team
      </label>
      <div className="flex gap-2">
        <Button type="button" variant="quiet" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" busy={save.isPending}>
          Save View
        </Button>
      </div>
      <div className="basis-full empty:hidden">
        <FormError error={save.error} />
      </div>
    </form>
  );
}

function RemoveView({ view }: { view: SavedView }) {
  const invalidate = useInvalidate();
  const remove = useMutation({
    mutationFn: () => del(`/views/${view.id}`),
    onSuccess: () => invalidate("view"),
  });
  return (
    <button
      type="button"
      aria-label={`Delete view ${view.name}`}
      disabled={remove.isPending}
      onClick={() => {
        if (window.confirm(`Delete the view “${view.name}”?`)) remove.mutate();
      }}
      className="text-muted hover:text-ink -ml-1 inline-flex size-8 items-center justify-center rounded-full"
    >
      <IconX size={14} stroke={2} aria-hidden="true" />
    </button>
  );
}
