"use client";

import { useMutation, useQuery } from "@tanstack/react-query";
import Link from "next/link";
import { useState } from "react";

import {
  Button,
  EmptyState,
  ErrorState,
  FormError,
  Loading,
  PageHeader,
  Panel,
  Pill,
  cx,
} from "@/components/ui";
import { get, post } from "@/lib/api";
import { percent } from "@/lib/format";
import { useInvalidate } from "@/lib/queries";
import { BACK } from "@/lib/transitions";
import type { DuplicatePair } from "@/lib/types";

type Kind = "contacts" | "companies";
type Row = DuplicatePair["first"];

const FIELDS: Record<Kind, { key: string; label: string; show?: string }[]> = {
  contacts: [
    { key: "first_name", label: "First name" },
    { key: "last_name", label: "Last name" },
    { key: "email", label: "Email" },
    { key: "phone", label: "Phone" },
    { key: "job_title", label: "Job title" },
    { key: "company_id", label: "Company", show: "company_name" },
    { key: "owner_id", label: "Owner", show: "owner_name" },
  ],
  companies: [
    { key: "name", label: "Name" },
    { key: "domain", label: "Domain" },
    { key: "industry", label: "Industry" },
    { key: "owner_id", label: "Owner", show: "owner_name" },
  ],
};

const title = (kind: Kind, row: Row) =>
  String(kind === "contacts" ? `${row.first_name} ${row.last_name}`.trim() : row.name);

export function Duplicates({ kind }: { kind: Kind }) {
  const pairs = useQuery({
    queryKey: ["duplicates", kind],
    queryFn: () => get<DuplicatePair[]>(`/duplicates/${kind}`),
  });
  return (
    <>
      <Link
        transitionTypes={BACK}
        href={`/${kind}`}
        className="text-body text-muted hover:text-ink underline underline-offset-4"
      >
        {kind === "contacts" ? "Contacts" : "Companies"}
      </Link>
      <PageHeader
        title={kind === "contacts" ? "Possible Duplicate Contacts" : "Possible Duplicate Companies"}
        description={`Pairs with the same ${kind === "contacts" ? "email" : "domain"} or very similar names. A merge keeps one record, moves every deal, follow-up and note onto it, fills its blanks and deletes the other.`}
      />
      {pairs.isPending ? (
        <Loading label="Looking for duplicates…" />
      ) : pairs.isError ? (
        <ErrorState error={pairs.error} onRetry={() => pairs.refetch()} />
      ) : pairs.data.length === 0 ? (
        <Panel>
          <EmptyState
            title="No duplicates found"
            body="Every record looks unique. Imports and new records are checked again each time you open this page."
          />
        </Panel>
      ) : (
        <div className="flex flex-col gap-4">
          {pairs.data.map((pair) => (
            <PairCard key={`${pair.first.id}-${pair.second.id}`} kind={kind} pair={pair} />
          ))}
        </div>
      )}
    </>
  );
}

function PairCard({ kind, pair }: { kind: Kind; pair: DuplicatePair }) {
  const invalidate = useInvalidate();
  const [keepFirst, setKeepFirst] = useState(true);
  const [take, setTake] = useState<string[]>([]);
  const [keep, remove] = keepFirst ? [pair.first, pair.second] : [pair.second, pair.first];
  const sides = [pair.first, pair.second];
  const merge = useMutation({
    mutationFn: () =>
      post(`/duplicates/${kind}/merge`, { keep: keep.id, remove: remove.id, take }),
    onSuccess: () => invalidate(kind === "contacts" ? "contact" : "company"),
  });

  const chosen = (key: string, shown: string) =>
    take.includes(key) || keep[shown] === null || keep[shown] === "" ? remove : keep;
  const pick = (key: string, row: Row) =>
    setTake((list) =>
      row === remove ? [...new Set([...list, key])] : list.filter((k) => k !== key),
    );

  return (
    <Panel className="p-4">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="text-lead font-semibold">{title(kind, pair.first)}</h2>
        {pair.same_key ? (
          <Pill tone="accent">Same {kind === "contacts" ? "email" : "domain"}</Pill>
        ) : (
          <Pill tone="open">{percent(pair.similarity)} similar name</Pill>
        )}
      </div>
      <div className="mt-3 overflow-x-auto">
        <table className="text-body w-full min-w-[32rem] border-separate border-spacing-y-1.5 text-left">
          <thead>
            <tr className="text-meta text-muted">
              <th scope="col" className="w-36 px-3 font-medium">
                Field
              </th>
              {sides.map((row, i) => (
                <th key={String(row.id)} scope="col" className="px-3 font-medium">
                  <label className="flex items-center gap-2">
                    <input
                      type="radio"
                      name={`keep-${pair.first.id}`}
                      checked={keepFirst === (i === 0)}
                      onChange={() => {
                        setKeepFirst(i === 0);
                        setTake([]);
                      }}
                      className="size-4 accent-[var(--accent)]"
                    />
                    {keepFirst === (i === 0) ? "Keep this record" : "Merge into the other"}
                  </label>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {FIELDS[kind].map((field) => {
              const values = sides.map((row) => row[field.show ?? field.key]);
              const differ = values[0] !== values[1];
              return (
                <tr key={field.key}>
                  <th scope="row" className="bg-row text-muted rounded-l-lg px-3 py-2.5 font-medium">
                    {field.label}
                  </th>
                  {sides.map((row, i) => {
                    const value = values[i];
                    const selected = chosen(field.key, field.show ?? field.key) === row;
                    return (
                      <td
                        key={String(row.id)}
                        className={cx(
                          "bg-row px-3 py-2.5 last:rounded-r-lg",
                          differ && selected && "text-ink font-medium",
                          differ && !selected && "text-muted",
                        )}
                      >
                        {differ && value !== null && value !== "" ? (
                          <label className="flex cursor-pointer items-center gap-2">
                            <input
                              type="radio"
                              name={`${pair.first.id}-${field.key}`}
                              checked={selected}
                              onChange={() => pick(field.key, row)}
                              className="size-4 shrink-0 accent-[var(--accent)]"
                            />
                            <span className="break-all">{String(value)}</span>
                          </label>
                        ) : (
                          <span className={cx("break-all", value === null && "text-muted")}>
                            {value === null || value === "" ? "empty" : String(value)}
                          </span>
                        )}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="mt-3 flex flex-wrap items-center justify-end gap-3">
        <FormError error={merge.error} />
        <Button variant="primary" busy={merge.isPending} onClick={() => merge.mutate()}>
          Merge into {title(kind, keep)}
        </Button>
      </div>
    </Panel>
  );
}
