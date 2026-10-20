"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useState } from "react";

import { ActivityPanel } from "@/components/activity";
import { DealFormDialog } from "@/components/deal-form";
import { NotesPanel } from "@/components/notes";
import { StageRail } from "@/components/stage-rail";
import { TaskFormDialog } from "@/components/task-form";
import { TaskRow } from "@/components/task-list";
import { Button, ErrorState, Loading, Panel, RecordView } from "@/components/ui";
import { del, get, query } from "@/lib/api";
import { dateTime, shortDate } from "@/lib/format";
import { useMoney } from "@/lib/hooks";
import { canModify, isManager, keys, useMe } from "@/lib/queries";
import type { Deal, Page, StageChange, Task } from "@/lib/types";

export default function DealPage() {
  const { id } = useParams<{ id: string }>();
  const deal = useQuery({ queryKey: keys.deal(id), queryFn: () => get<Deal>(`/deals/${id}`) });
  return (
    <RecordView
      query={deal}
      missing={{
        title: "This deal no longer exists",
        href: "/pipeline",
        back: "Back to the pipeline",
      }}
    >
      {(deal) => <DealView deal={deal} />}
    </RecordView>
  );
}

function DealView({ deal }: { deal: Deal }) {
  const me = useMe().data;
  const fmt = useMoney();
  const [editing, setEditing] = useState(false);
  const [addingTask, setAddingTask] = useState(false);
  const editable = canModify(me, deal.owner_id);

  return (
    <>
      <Link
        href="/pipeline"
        className="text-body text-muted hover:text-ink underline underline-offset-4"
      >
        Pipeline
      </Link>
      <header className="mt-3 flex flex-wrap items-start justify-between gap-x-8 gap-y-4 pb-6">
        <div className="min-w-0">
          <h1 className="text-display font-semibold tracking-tight break-words">{deal.title}</h1>
          <p className="text-muted mt-1">
            {deal.company_id ? (
              <Link
                href={`/companies/${deal.company_id}`}
                className="hover:text-ink underline underline-offset-4"
              >
                {deal.company_name}
              </Link>
            ) : (
              "No company linked"
            )}
          </p>
          <p className="num text-display mt-4 font-semibold">{fmt(deal.value_cents)}</p>
        </div>
        {editable ? (
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => setEditing(true)}>Edit deal</Button>
            {isManager(me) ? <DeleteDeal deal={deal} /> : null}
          </div>
        ) : (
          <p className="text-meta text-muted max-w-xs">
            Owned by {deal.owner_name}. Only the owner or a manager can change it.
          </p>
        )}
      </header>

      <StageRail deal={deal} editable={editable} />

      <div className="mt-6 grid items-start gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
        <div className="flex flex-col gap-6">
          <DealTasks dealId={deal.id} onAdd={() => setAddingTask(true)} />
          <NotesPanel parent={{ deal_id: deal.id }} />
        </div>
        <div className="flex flex-col gap-6">
          <Panel quiet>
            <h2 className="text-title px-4 pt-4 font-semibold">Details</h2>
            <dl className="text-body grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 p-4">
              <dt className="text-muted">Contact</dt>
              <dd className="min-w-0 truncate">
                {deal.contact_id ? (
                  <Link
                    href={`/contacts/${deal.contact_id}`}
                    className="underline underline-offset-4"
                  >
                    {deal.contact_name}
                  </Link>
                ) : (
                  "None"
                )}
              </dd>
              <dt className="text-muted">Owner</dt>
              <dd>{deal.owner_name}</dd>
              <dt className="text-muted">Expected close</dt>
              <dd className="num">
                {deal.expected_close_date ? shortDate(deal.expected_close_date) : "Not set"}
              </dd>
              <dt className="text-muted">Created</dt>
              <dd className="num">{dateTime(deal.created_at)}</dd>
              {deal.closed_at ? (
                <>
                  <dt className="text-muted">Closed</dt>
                  <dd className="num">{dateTime(deal.closed_at)}</dd>
                </>
              ) : null}
            </dl>
          </Panel>
          <StageHistory dealId={deal.id} />
          <ActivityPanel entityType="deal" entityId={deal.id} />
        </div>
      </div>

      <DealFormDialog open={editing} onClose={() => setEditing(false)} deal={deal} />
      <TaskFormDialog
        open={addingTask}
        onClose={() => setAddingTask(false)}
        link={{ dealId: deal.id }}
      />
    </>
  );
}

function DeleteDeal({ deal }: { deal: Deal }) {
  const router = useRouter();
  const client = useQueryClient();
  const remove = useMutation({
    mutationFn: () => del(`/deals/${deal.id}`),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: ["board"] });
      client.invalidateQueries({ queryKey: ["deals"] });
      client.invalidateQueries({ queryKey: ["analytics"] });
      router.push("/pipeline");
    },
  });
  return (
    <Button
      variant="danger"
      busy={remove.isPending}
      onClick={() => {
        if (window.confirm(`Delete "${deal.title}"? Its notes and follow-ups are deleted too.`))
          remove.mutate();
      }}
    >
      Delete
    </Button>
  );
}

function DealTasks({ dealId, onAdd }: { dealId: string; onAdd: () => void }) {
  const tasks = useQuery({
    queryKey: keys.tasks({ deal: dealId }),
    queryFn: () => get<Page<Task>>(`/tasks${query({ deal_id: dealId, per_page: 50 })}`),
  });
  return (
    <Panel>
      <div className="flex items-center justify-between gap-3 px-4 pt-4">
        <h2 className="text-title font-semibold">Follow-ups</h2>
        <Button variant="quiet" onClick={onAdd}>
          Add follow-up
        </Button>
      </div>
      {tasks.isPending ? (
        <Loading label="Loading follow-ups…" />
      ) : tasks.isError ? (
        <ErrorState error={tasks.error} onRetry={() => tasks.refetch()} />
      ) : tasks.data.data.length === 0 ? (
        <p className="text-body text-muted px-4 py-6">
          No follow-ups planned. Add the next step so this deal does not stall.
        </p>
      ) : (
        <ul className="divide-line border-line mt-2 divide-y border-t">
          {tasks.data.data.map((task) => (
            <TaskRow key={task.id} task={task} showLinks={false} />
          ))}
        </ul>
      )}
    </Panel>
  );
}

function StageHistory({ dealId }: { dealId: string }) {
  const history = useQuery({
    queryKey: keys.stageHistory(dealId),
    queryFn: () => get<StageChange[]>(`/deals/${dealId}/stage-history`),
  });
  return (
    <Panel quiet>
      <h2 className="text-title px-4 pt-4 font-semibold">Stage path</h2>
      {history.isPending ? (
        <Loading label="Loading stage path…" />
      ) : history.isError ? (
        <ErrorState error={history.error} onRetry={() => history.refetch()} />
      ) : (
        <ol className="flex flex-col gap-2 p-4">
          {history.data.map((step, i) => (
            <li key={i} className="flex items-baseline justify-between gap-3">
              <span>
                {step.from_stage_name ? `${step.from_stage_name} to ` : "Created in "}
                <span className="font-medium">{step.to_stage_name}</span>
              </span>
              <span className="num text-meta text-muted shrink-0">
                {shortDate(step.changed_at)}
              </span>
            </li>
          ))}
        </ol>
      )}
    </Panel>
  );
}
