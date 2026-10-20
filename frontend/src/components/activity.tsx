"use client";

import { useQuery } from "@tanstack/react-query";

import { ErrorState, Loading, Panel } from "@/components/ui";
import { get, query } from "@/lib/api";
import { dateTime } from "@/lib/format";
import { useMoney } from "@/lib/hooks";
import { keys, useStages, useUsers } from "@/lib/queries";
import type { AuditEntry, Page } from "@/lib/types";

const FIELD_LABELS: Record<string, string> = {
  title: "name",
  value_cents: "value",
  stage_id: "stage",
  company_id: "company",
  contact_id: "contact",
  owner_id: "owner",
  expected_close_date: "expected close",
  first_name: "first name",
  last_name: "last name",
  job_title: "job title",
  assignee_id: "assignee",
  due_at: "due date",
  completed_at: "completion",
};

export function ActivityPanel({ entityType, entityId }: { entityType: string; entityId: string }) {
  const entries = useQuery({
    queryKey: keys.audit({ entityType, entityId }),
    queryFn: () =>
      get<Page<AuditEntry>>(
        `/audit${query({ entity_type: entityType, entity_id: entityId, per_page: 30 })}`,
      ),
  });

  return (
    <Panel>
      <h2 className="text-title px-4 pt-4 font-semibold">History</h2>
      {entries.isPending ? (
        <Loading label="Loading history…" />
      ) : entries.isError ? (
        <ErrorState error={entries.error} onRetry={() => entries.refetch()} />
      ) : entries.data.data.length === 0 ? (
        <p className="text-body text-muted px-4 py-6">No recorded changes yet.</p>
      ) : (
        <ol className="flex flex-col gap-3 p-4">
          {entries.data.data.map((entry) => (
            <li key={entry.id} className="border-line border-l-2 pl-3">
              <EntrySentence entry={entry} />
              <p className="num text-meta text-muted mt-0.5">{dateTime(entry.created_at)}</p>
            </li>
          ))}
        </ol>
      )}
    </Panel>
  );
}

function EntrySentence({ entry }: { entry: AuditEntry }) {
  const actor = entry.actor_name ?? "Someone";
  if (entry.action === "created") return <p>{actor} created this record.</p>;
  if (entry.action === "deleted") return <p>{actor} deleted this record.</p>;
  const fields = Object.entries(entry.changes);
  return (
    <div>
      <p>{actor} changed:</p>
      <ul className="text-body text-muted mt-1">
        {fields.map(([field, change]) => (
          <li key={field}>
            <ChangeLine field={field} change={change as { from: unknown; to: unknown }} />
          </li>
        ))}
      </ul>
    </div>
  );
}

function ChangeLine({ field, change }: { field: string; change: { from: unknown; to: unknown } }) {
  const fmt = useMoney();
  const users = useUsers().data ?? [];
  const stages = useStages().data ?? [];

  const show = (value: unknown): string => {
    if (value === null || value === undefined || value === "") return "empty";
    if (field === "value_cents" && typeof value === "number") return fmt(value);
    if ((field === "owner_id" || field === "assignee_id") && typeof value === "string")
      return users.find((u) => u.id === value)?.full_name ?? "a former teammate";
    if (field === "stage_id" && typeof value === "string")
      return stages.find((s) => s.id === value)?.name ?? "a removed stage";
    if (field.endsWith("_id")) return "another record";
    if ((field === "due_at" || field === "completed_at") && typeof value === "string")
      return dateTime(value);
    return String(value);
  };

  return (
    <>
      {FIELD_LABELS[field] ?? field.replaceAll("_", " ")}:{" "}
      <span className="line-through decoration-1">{show(change.from)}</span>
      {" to "}
      <span className="text-ink">{show(change.to)}</span>
    </>
  );
}
