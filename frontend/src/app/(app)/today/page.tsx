"use client";

import { useQuery } from "@tanstack/react-query";
import Link from "next/link";
import { useState } from "react";

import { DealLink, DealName } from "@/components/deal-link";
import { StageTrack } from "@/components/stage-track";
import { TaskFormDialog } from "@/components/task-form";
import { TaskGroup } from "@/components/task-list";
import { Button, EmptyState, ErrorState, Loading, PageHeader, Panel, cx } from "@/components/ui";
import { get, query } from "@/lib/api";
import { parseDay, shortDate } from "@/lib/format";
import { useMoney } from "@/lib/hooks";
import { keys, useMe, useStages } from "@/lib/queries";
import type { Deal, Page, Summary, Task } from "@/lib/types";
import { SECTION } from "@/lib/transitions";

function groupTasks(tasks: Task[], now = new Date()) {
  const endOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  const endOfWeek = new Date(endOfToday.getTime() + 6 * 86_400_000);
  const groups = {
    overdue: [] as Task[],
    today: [] as Task[],
    week: [] as Task[],
    later: [] as Task[],
  };
  for (const task of tasks) {
    const due = task.due_at ? new Date(task.due_at) : null;
    if (!due) groups.later.push(task);
    else if (due < now) groups.overdue.push(task);
    else if (due < endOfToday) groups.today.push(task);
    else if (due < endOfWeek) groups.week.push(task);
    else groups.later.push(task);
  }
  return groups;
}

export default function TodayPage() {
  const [adding, setAdding] = useState(false);
  const todayLabel = new Intl.DateTimeFormat(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
  }).format(new Date());

  return (
    <>
      <PageHeader
        title="Today"
        description={<time suppressHydrationWarning>{todayLabel}</time>}
        actions={
          <Button variant="primary" onClick={() => setAdding(true)}>
            Add Follow-Up
          </Button>
        }
      />
      <MyNumbers />
      <div className="mt-6 grid items-start gap-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <FollowUps onAdd={() => setAdding(true)} />
        <ClosingSoon />
      </div>
      <TaskFormDialog open={adding} onClose={() => setAdding(false)} />
    </>
  );
}

function MyNumbers() {
  const me = useMe().data;
  const fmt = useMoney();
  const summary = useQuery({
    queryKey: keys.analytics("summary", { owner: me?.id }),
    queryFn: () => get<Summary>(`/analytics/summary${query({ owner_id: me?.id })}`),
    enabled: !!me,
  });
  if (!summary.data) return null;
  const s = summary.data;
  return (
    <p className="text-title text-muted font-display max-w-3xl">
      You have <strong className="num text-ink font-semibold">{fmt(s.open_pipeline_cents)}</strong>{" "}
      open across <span className="num">{s.open_deals_count}</span>{" "}
      {s.open_deals_count === 1 ? "deal" : "deals"}, about{" "}
      <strong className="num text-ink font-semibold">{fmt(s.weighted_pipeline_cents)}</strong> once
      weighted by stage.{" "}
      {s.won_this_month_count > 0 ? (
        <>
          You closed{" "}
          <strong className="num text-accent font-semibold">{fmt(s.won_this_month_cents)}</strong>{" "}
          this month.
        </>
      ) : (
        "Nothing won yet this month."
      )}
    </p>
  );
}

function FollowUps({ onAdd }: { onAdd: () => void }) {
  const me = useMe().data;
  const tasks = useQuery({
    queryKey: keys.tasks({ assignee: me?.id, status: "open", view: "today" }),
    queryFn: () =>
      get<Page<Task>>(`/tasks${query({ assignee_id: me?.id, status: "open", per_page: 100 })}`),
    enabled: !!me,
  });

  return (
    <Panel>
      <h2 className="text-title px-4 pt-4 font-semibold">Your Follow-Ups</h2>
      {tasks.isPending ? (
        <Loading label="Loading your follow-ups…" />
      ) : tasks.isError ? (
        <ErrorState error={tasks.error} onRetry={() => tasks.refetch()} />
      ) : tasks.data.data.length === 0 ? (
        <EmptyState
          title="No open follow-ups"
          body="Add a follow-up for the next call, email or proposal so it shows up here on the day it is due."
          action={<Button onClick={onAdd}>Add Follow-Up</Button>}
        />
      ) : (
        <TaskSections tasks={tasks.data.data} />
      )}
    </Panel>
  );
}

function TaskSections({ tasks }: { tasks: Task[] }) {
  const groups = groupTasks(tasks);
  return (
    <div className="pb-2">
      <TaskGroup title="Overdue" tasks={groups.overdue} tone="critical" />
      <TaskGroup title="Due Today" tasks={groups.today} />
      <TaskGroup title="Next 7 Days" tasks={groups.week} />
      <TaskGroup title="Later or Undated" tasks={groups.later} />
    </div>
  );
}

function ClosingSoon() {
  const me = useMe().data;
  const stages = useStages().data ?? [];
  const fmt = useMoney();
  const deals = useQuery({
    queryKey: keys.deals({ owner: me?.id, status: "open", view: "today" }),
    queryFn: () =>
      get<Page<Deal>>(`/deals${query({ owner_id: me?.id, status: "open", per_page: 100 })}`),
    enabled: !!me,
  });

  const now = new Date();
  const monthEnd = new Date(now.getFullYear(), now.getMonth() + 1, 1);
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const dated = (deals.data?.data ?? [])
    .filter((d) => d.expected_close_date && parseDay(d.expected_close_date) < monthEnd)
    .sort((a, b) => (a.expected_close_date ?? "").localeCompare(b.expected_close_date ?? ""));

  return (
    <Panel>
      <div className="flex items-baseline justify-between gap-3 px-4 pt-4">
        <h2 className="text-title font-semibold">Closing This Month</h2>
        <Link transitionTypes={SECTION} href="/pipeline" className="text-body text-accent underline underline-offset-4">
          Open pipeline
        </Link>
      </div>
      <p className="text-meta text-muted px-4 pt-1">
        Your open deals expected to close by the end of the month, including ones that slipped.
      </p>
      {deals.isPending ? (
        <Loading label="Loading deals…" />
      ) : deals.isError ? (
        <ErrorState error={deals.error} onRetry={() => deals.refetch()} />
      ) : dated.length === 0 ? (
        <EmptyState
          title="Nothing due to close this month"
          body="Set an expected close date on a deal to plan the month around it."
        />
      ) : (
        <ul className="divide-line border-line mt-3 divide-y border-t">
          {dated.map((deal) => {
            const slipped = parseDay(deal.expected_close_date!) < startOfToday;
            return (
              <li key={deal.id}>
                <DealLink deal={deal}
                  className="hover:bg-hover flex items-center gap-3 px-4 py-3"
                >
                  <div className="min-w-0 flex-1">
                    <DealName deal={deal} className="block truncate font-medium" />
                    <p className="text-meta text-muted mt-1 flex items-center gap-2">
                      <StageTrack stages={stages} stageId={deal.stage_id} kind={deal.stage_kind} />
                      <span className="truncate">{deal.company_name ?? deal.stage_name}</span>
                    </p>
                  </div>
                  <div className="shrink-0 text-right">
                    <p className="num font-medium">{fmt(deal.value_cents)}</p>
                    <p className={cx("num text-meta", slipped ? "text-critical" : "text-muted")}>
                      {slipped ? "Slipped from " : ""}
                      {shortDate(deal.expected_close_date!)}
                    </p>
                  </div>
                </DealLink>
              </li>
            );
          })}
        </ul>
      )}
    </Panel>
  );
}
