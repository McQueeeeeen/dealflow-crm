"use client";

import { IconBriefcase, IconScale, IconTrophy } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import Link from "next/link";
import { useState } from "react";

import { DealLink, DealName } from "@/components/deal-link";
import { StageTrack } from "@/components/stage-track";
import { TaskFormDialog } from "@/components/task-form";
import { TaskGroup } from "@/components/task-list";
import {
  Button,
  EmptyState,
  ErrorState,
  Loading,
  PageHeader,
  Panel,
  Stat,
  TickMeter,
  cx,
} from "@/components/ui";
import { get, query } from "@/lib/api";
import { parseDay, percent, shortDate } from "@/lib/format";
import { stageHeat } from "@/lib/heat";
import { useMoney } from "@/lib/hooks";
import { keys, useMe, useMyTasks, useStages } from "@/lib/queries";
import type { BoardColumn, Deal, Page, Summary, Task } from "@/lib/types";
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
  const me = useMe().data;
  const tasks = useMyTasks().data?.data;
  const groups = tasks ? groupTasks(tasks) : null;

  return (
    <>
      <PageHeader
        title={`Welcome back, ${me?.full_name.split(" ")[0] ?? ""}`}
        description={
          groups ? (
            <>
              <strong className="num text-ink font-semibold">{groups.today.length}</strong>{" "}
              {groups.today.length === 1 ? "follow-up" : "follow-ups"} due today,{" "}
              <strong
                className={cx(
                  "num font-semibold",
                  groups.overdue.length > 0 ? "text-critical" : "text-ink",
                )}
              >
                {groups.overdue.length}
              </strong>{" "}
              overdue.
            </>
          ) : (
            "Loading your day…"
          )
        }
        actions={
          <Button variant="primary" onClick={() => setAdding(true)}>
            Add Follow-Up
          </Button>
        }
      />
      <MyNumbers />
      <div className="mt-6 grid items-start gap-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <FollowUps onAdd={() => setAdding(true)} />
        <div className="grid gap-6">
          <StageBreakdown />
          <ClosingSoon />
        </div>
      </div>
      <TaskFormDialog open={adding} onClose={() => setAdding(false)} />
    </>
  );
}

function useMyBoard() {
  const me = useMe().data;
  return useQuery({
    queryKey: keys.board({ owner: me?.id, view: "today" }),
    queryFn: () => get<BoardColumn[]>(`/deals/board${query({ owner_id: me?.id })}`),
    enabled: !!me,
  });
}

function MyNumbers() {
  const me = useMe().data;
  const fmt = useMoney();
  const stages = useStages().data ?? [];
  const open = (useMyBoard().data ?? []).filter((c) => c.stage.kind === "open");
  const summary = useQuery({
    queryKey: keys.analytics("summary", { owner: me?.id }),
    queryFn: () => get<Summary>(`/analytics/summary${query({ owner_id: me?.id })}`),
    enabled: !!me,
  });
  if (!summary.data) return null;
  const s = summary.data;
  const weightedShare = s.weighted_pipeline_cents / Math.max(s.open_pipeline_cents, 1);
  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 [&>:first-child]:col-span-2 sm:[&>:first-child]:col-span-1">
      <Stat
        label="Your open pipeline"
        icon={IconBriefcase}
        value={fmt(s.open_pipeline_cents)}
        meter={
          <TickMeter
            label="Open value split by stage"
            segments={open.map((c) => ({
              value: c.total_value_cents,
              color: stageHeat(stages, c.stage),
            }))}
          />
        }
        note={`${s.open_deals_count} open ${s.open_deals_count === 1 ? "deal" : "deals"}, split by stage`}
      />
      <Stat
        label="Weighted by stage"
        icon={IconScale}
        value={fmt(s.weighted_pipeline_cents)}
        meter={<TickMeter label={`${percent(weightedShare)} of open value`} value={weightedShare} />}
        note={`${percent(weightedShare)} of your open value`}
      />
      <Stat
        label="Won this month"
        icon={IconTrophy}
        value={
          <span className={s.won_this_month_count > 0 ? "text-won" : undefined}>
            {fmt(s.won_this_month_cents)}
          </span>
        }
        meter={
          <TickMeter
            label={`Win rate ${percent(s.win_rate_90d)}`}
            value={s.win_rate_90d ?? 0}
            color="var(--won)"
          />
        }
        note={`${s.won_this_month_count} ${s.won_this_month_count === 1 ? "deal" : "deals"}, ${percent(s.win_rate_90d)} win rate over 90 days`}
      />
    </div>
  );
}

function StageBreakdown() {
  const board = useMyBoard();
  const stages = useStages().data ?? [];
  const fmt = useMoney();
  const open = (board.data ?? []).filter((c) => c.stage.kind === "open");
  const top = Math.max(...open.map((c) => c.total_value_cents), 1);
  return (
    <Panel>
      <h2 className="text-title px-4 pt-4 font-semibold">Your Pipeline by Stage</h2>
      {board.isPending ? (
        <Loading label="Loading stages…" />
      ) : board.isError ? (
        <ErrorState error={board.error} onRetry={() => board.refetch()} />
      ) : (
        <ul className="row-list mt-1">
          {open.map((c) => {
            const color = stageHeat(stages, c.stage);
            return (
              <li key={c.stage.id} className="flex flex-col gap-2.5 px-3.5 py-3">
                <div className="flex items-baseline justify-between gap-3">
                  <span className="flex items-center gap-2 font-medium">
                    <span
                      aria-hidden="true"
                      className="size-2 rounded-full"
                      style={{ backgroundColor: color }}
                    />
                    {c.stage.name}
                  </span>
                  <span className="num text-meta text-muted">
                    <span className="text-body text-ink font-medium">
                      {fmt(c.total_value_cents)}
                    </span>{" "}
                    in {c.count} {c.count === 1 ? "deal" : "deals"}
                  </span>
                </div>
                <TickMeter
                  label={`${c.stage.name}: ${fmt(c.total_value_cents)}`}
                  value={c.total_value_cents / top}
                  color={color}
                  ticks={48}
                />
              </li>
            );
          })}
        </ul>
      )}
    </Panel>
  );
}

function FollowUps({ onAdd }: { onAdd: () => void }) {
  const tasks = useMyTasks();

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
        <Link
          transitionTypes={SECTION}
          href="/pipeline"
          className="text-body text-link underline underline-offset-4"
        >
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
        <ul className="row-list mt-1">
          {dated.map((deal) => {
            const slipped = parseDay(deal.expected_close_date!) < startOfToday;
            return (
              <li key={deal.id}>
                <DealLink deal={deal} className="hover:bg-hover flex items-center gap-3 px-4 py-3">
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
