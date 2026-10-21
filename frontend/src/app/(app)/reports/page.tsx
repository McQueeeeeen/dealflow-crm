"use client";

import {
  IconBriefcase,
  IconClockHour4,
  IconPercentage,
  IconScale,
  IconTrophy,
} from "@tabler/icons-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

import { OwnerFilter } from "@/components/owner-select";
import { Table } from "@/components/table";
import {
  EmptyState,
  ErrorState,
  Loading,
  PageHeader,
  Panel,
  Button,
  Person,
  Select,
  Stat,
  TextInput,
  TickMeter,
} from "@/components/ui";
import { get, put, query } from "@/lib/api";
import { centsToInput, monthLabel, parseMoneyInput, percent, shortDate } from "@/lib/format";
import { WON_HEAT } from "@/lib/heat";
import { useMoney, useUrlParams } from "@/lib/hooks";
import { isManager, keys, useInvalidate, useMe, useTargets } from "@/lib/queries";
import type { Forecast, Funnel, LeaderboardRow, Summary, TargetRow } from "@/lib/types";

import { FunnelChart, ForecastChart } from "./charts";
import { SimulationSection } from "./simulation";

const PERIODS = [
  { days: 90, label: "Last 90 days" },
  { days: 180, label: "Last 6 months" },
  { days: 365, label: "Last 12 months" },
];

function sinceDate(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() - days);
  return d.toISOString().slice(0, 10);
}

export default function ReportsPage() {
  const me = useMe().data;
  const [params, update] = useUrlParams();
  const owner = params.get("owner") ?? "";
  const days = Number(params.get("days") ?? 180);
  const from = sinceDate(days);

  return (
    <>
      <PageHeader
        title="Reports"
        description="Built from the live pipeline and every stage change your team has recorded."
      />
      <div className="mb-6 flex flex-wrap items-center gap-x-5 gap-y-3">
        <OwnerFilter value={owner} onChange={(v) => update({ owner: v })} label="Deals of" />
        <label className="flex items-center gap-2">
          <span className="text-muted">Period</span>
          <Select
            value={days}
            onChange={(e) => update({ days: e.target.value === "180" ? null : e.target.value })}
            className="w-auto"
          >
            {PERIODS.map((p) => (
              <option key={p.days} value={p.days}>
                {p.label}
              </option>
            ))}
          </Select>
        </label>
      </div>

      <SummaryStrip owner={owner} />
      <SimulationSection owner={owner} />

      <div className="mt-6 grid items-start gap-6 xl:grid-cols-2">
        <FunnelSection owner={owner} from={from} />
        <ForecastSection owner={owner} />
      </div>

      <TargetsSection />
      {isManager(me) ? <LeaderboardSection from={from} owner={owner} /> : null}
    </>
  );
}

function Section({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: ReactNode;
}) {
  return (
    <Panel className="p-4 sm:p-5">
      <h2 className="text-title font-semibold">{title}</h2>
      <p className="text-body text-muted mt-1 mb-5">{subtitle}</p>
      {children}
    </Panel>
  );
}

function SummaryStrip({ owner }: { owner: string }) {
  const fmt = useMoney();
  const summary = useQuery({
    queryKey: keys.analytics("summary", { owner }),
    queryFn: () => get<Summary>(`/analytics/summary${query({ owner_id: owner })}`),
  });
  if (summary.isPending) return <Loading label="Loading figures…" />;
  if (summary.isError)
    return <ErrorState error={summary.error} onRetry={() => summary.refetch()} />;
  const s = summary.data;
  const weightedShare = s.weighted_pipeline_cents / Math.max(s.open_pipeline_cents, 1);
  const figures = [
    {
      label: "Open pipeline",
      icon: IconBriefcase,
      value: fmt(s.open_pipeline_cents),
      note: `${s.open_deals_count} open deals`,
    },
    {
      label: "Weighted pipeline",
      icon: IconScale,
      value: fmt(s.weighted_pipeline_cents),
      meter: <TickMeter label={`${percent(weightedShare)} of open value`} value={weightedShare} />,
      note: `${percent(weightedShare)} of open value`,
    },
    {
      label: "Won this month",
      icon: IconTrophy,
      value: fmt(s.won_this_month_cents),
      note: `${s.won_this_month_count} deals`,
    },
    {
      label: "Win rate",
      icon: IconPercentage,
      value: percent(s.win_rate_90d),
      meter: (
        <TickMeter
          label={`Win rate ${percent(s.win_rate_90d)}`}
          value={s.win_rate_90d ?? 0}
          color="var(--won)"
        />
      ),
      note: "Closed deals, last 90 days",
    },
    {
      label: "Time to win",
      icon: IconClockHour4,
      value: s.avg_days_to_win === null ? "n/a" : `${Math.round(s.avg_days_to_win)} days`,
      note: "Average, last 180 days",
    },
  ];
  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
      {figures.map((f) => (
        <Stat key={f.label} {...f} />
      ))}
    </div>
  );
}

function FunnelSection({ owner, from }: { owner: string; from: string }) {
  const funnel = useQuery({
    queryKey: keys.analytics("funnel", { owner, from }),
    queryFn: () => get<Funnel>(`/analytics/funnel${query({ owner_id: owner, from })}`),
  });
  return (
    <Section
      title="Where Do Deals Drop Off?"
      subtitle={
        funnel.data
          ? `${funnel.data.cohort_size} deals created since ${shortDate(funnel.data.from)}, counted at the furthest stage they reached.`
          : "Deals created in the period, counted at the furthest stage they reached."
      }
    >
      {funnel.isPending ? (
        <Loading label="Loading funnel…" />
      ) : funnel.isError ? (
        <ErrorState error={funnel.error} onRetry={() => funnel.refetch()} />
      ) : funnel.data.cohort_size === 0 ? (
        <EmptyState
          title="No deals created in this period"
          body="Pick a longer period or another owner."
        />
      ) : (
        <FunnelChart stages={funnel.data.stages} cohort={funnel.data.cohort_size} />
      )}
    </Section>
  );
}

function ForecastSection({ owner }: { owner: string }) {
  const fmt = useMoney();
  const forecast = useQuery({
    queryKey: keys.analytics("forecast", { owner }),
    queryFn: () => get<Forecast>(`/analytics/forecast${query({ owner_id: owner, months: 6 })}`),
  });
  return (
    <Section
      title="What Could Close in the Next Six Months?"
      subtitle="Open deals grouped by their expected close date."
    >
      {forecast.isPending ? (
        <Loading label="Loading forecast…" />
      ) : forecast.isError ? (
        <ErrorState error={forecast.error} onRetry={() => forecast.refetch()} />
      ) : (
        <>
          <ForecastChart months={forecast.data.months} />
          {forecast.data.overdue_cents > 0 || forecast.data.undated_cents > 0 ? (
            <p className="num text-body text-muted mt-4">
              Not in the chart:{" "}
              <span className="text-critical">
                {fmt(forecast.data.overdue_cents)} past its expected date
              </span>
              {" and "}
              {fmt(forecast.data.undated_cents)} with no date set.
            </p>
          ) : null}
        </>
      )}
    </Section>
  );
}

function LeaderboardSection({ from, owner }: { from: string; owner: string }) {
  const fmt = useMoney();
  const board = useQuery({
    queryKey: keys.analytics("leaderboard", { from, owner }),
    queryFn: () =>
      get<LeaderboardRow[]>(`/analytics/leaderboard${query({ from, owner_id: owner })}`),
  });
  const top = Math.max(...(board.data ?? []).map((r) => r.won_cents), 1);
  return (
    <div className="mt-6">
      <Section title="Who Is Closing?" subtitle="Won value in the selected period, by deal owner.">
        {board.isPending ? (
          <Loading label="Loading team results…" />
        ) : board.isError ? (
          <ErrorState error={board.error} onRetry={() => board.refetch()} />
        ) : (
          <div className="-mx-4 sm:-mx-5">
            <Table
              caption="Results by owner"
              rows={board.data.map((r) => ({ ...r, id: r.user_id }))}
              columns={[
                {
                  header: "Owner",
                  cell: (r) => (
                    <span className="font-medium">
                      <Person name={r.full_name} />
                    </span>
                  ),
                },
                {
                  header: "Won",
                  cell: (r) => (
                    <div className="flex items-center gap-3">
                      <span className="num w-24 shrink-0 text-right">{fmt(r.won_cents)}</span>
                      <span className="hidden h-3 flex-1 sm:block" aria-hidden="true">
                        <span
                          className="block h-full rounded-full"
                          style={{
                            width: `${(r.won_cents / top) * 100}%`,
                            backgroundColor: WON_HEAT,
                          }}
                        />
                      </span>
                    </div>
                  ),
                },
                { header: "Deals won", cell: (r) => r.won_count, align: "right", hideBelow: "md" },
                { header: "Win rate", cell: (r) => percent(r.win_rate), align: "right" },
                {
                  header: "Open pipeline",
                  cell: (r) => fmt(r.open_pipeline_cents),
                  align: "right",
                  hideBelow: "sm",
                },
              ]}
            />
          </div>
        )}
      </Section>
    </div>
  );
}

function TargetsSection() {
  const me = useMe().data;
  const fmt = useMoney();
  const targets = useTargets();
  const data = targets.data;
  return (
    <div className="mt-6">
      <Section
        title="Monthly Targets"
        subtitle={`Won value against each target for ${data ? monthLabel(data.month) : "this month"}. Lighter ticks are open deals expected to close this month, weighted by stage.`}
      >
        {targets.isPending ? (
          <Loading label="Loading targets…" />
        ) : targets.isError ? (
          <ErrorState error={targets.error} onRetry={() => targets.refetch()} />
        ) : (
          <ul className="row-list -mx-2 p-0">
            {targets.data.rows.map((row) => {
              const goal = row.target_cents;
              return (
                <li
                  key={row.user_id}
                  className="grid items-center gap-x-6 gap-y-2 px-3.5 py-3 md:grid-cols-[11rem_minmax(0,1fr)_auto]"
                >
                  <span className="font-medium">
                    <Person name={row.full_name} />
                  </span>
                  <TickMeter
                    label={`${row.full_name}: ${fmt(row.won_cents)} won${goal ? ` of ${fmt(goal)}` : ""}`}
                    segments={[
                      { value: row.won_cents, color: "var(--won)" },
                      {
                        value: row.weighted_cents,
                        color: "color-mix(in oklch, var(--won) 35%, transparent)",
                      },
                    ]}
                    max={goal ?? undefined}
                    ticks={56}
                    className="h-5"
                  />
                  <div className="flex items-center justify-between gap-4 md:justify-end">
                    <p className="num text-body">
                      <span className="font-medium">{fmt(row.won_cents)}</span>{" "}
                      <span className="text-muted">
                        {goal ? `of ${fmt(goal)}, ${percent(row.won_cents / goal)}` : "no target"}
                      </span>
                    </p>
                    {isManager(me) ? <TargetEditor row={row} month={targets.data.month} /> : null}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </Section>
    </div>
  );
}

function TargetEditor({ row, month }: { row: TargetRow; month: string }) {
  const invalidate = useInvalidate();
  const [value, setValue] = useState(row.target_cents ? centsToInput(row.target_cents) : "");
  const save = useMutation({
    mutationFn: (amount: number | null) =>
      put(`/targets/${row.user_id}`, { month, amount_cents: amount }),
    onSuccess: () => invalidate("target"),
  });
  const cents = value.trim() ? parseMoneyInput(value) : null;
  return (
    <form
      className="flex items-center gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate(cents);
      }}
    >
      <label className="sr-only" htmlFor={`target-${row.user_id}`}>
        Target for {row.full_name}
      </label>
      <TextInput
        id={`target-${row.user_id}`}
        name="target"
        inputMode="decimal"
        placeholder="Target"
        value={value}
        aria-invalid={value.trim() !== "" && cents === null ? true : undefined}
        onChange={(e) => setValue(e.target.value)}
        className="num w-28"
      />
      <Button type="submit" busy={save.isPending} disabled={value.trim() !== "" && cents === null}>
        Set
      </Button>
    </form>
  );
}
