"use client";

import { useQuery } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

import { OwnerFilter } from "@/components/owner-select";
import { Table } from "@/components/table";
import { EmptyState, ErrorState, Loading, PageHeader, Panel, Select } from "@/components/ui";
import { get, query } from "@/lib/api";
import { percent, shortDate } from "@/lib/format";
import { useMoney } from "@/lib/hooks";
import { isManager, keys, useMe } from "@/lib/queries";
import type { Forecast, Funnel, LeaderboardRow, Summary } from "@/lib/types";

import { FunnelChart, ForecastChart } from "./charts";

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
  const [owner, setOwner] = useState("");
  const [days, setDays] = useState(180);
  const from = sinceDate(days);

  return (
    <>
      <PageHeader
        title="Reports"
        description="Built from the live pipeline and every stage change your team has recorded."
      />
      <div className="mb-6 flex flex-wrap items-center gap-x-5 gap-y-3">
        <OwnerFilter value={owner} onChange={setOwner} label="Deals of" />
        <label className="flex items-center gap-2">
          <span className="text-muted">Period</span>
          <Select value={days} onChange={(e) => setDays(Number(e.target.value))} className="w-auto">
            {PERIODS.map((p) => (
              <option key={p.days} value={p.days}>
                {p.label}
              </option>
            ))}
          </Select>
        </label>
      </div>

      <SummaryStrip owner={owner} />

      <div className="mt-6 grid items-start gap-6 xl:grid-cols-2">
        <FunnelSection owner={owner} from={from} />
        <ForecastSection owner={owner} />
      </div>

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
  const figures = [
    {
      label: "Open pipeline",
      value: fmt(s.open_pipeline_cents),
      note: `${s.open_deals_count} open deals`,
    },
    {
      label: "Weighted pipeline",
      value: fmt(s.weighted_pipeline_cents),
      note: "Value times stage probability",
    },
    {
      label: "Won this month",
      value: fmt(s.won_this_month_cents),
      note: `${s.won_this_month_count} deals`,
    },
    { label: "Win rate", value: percent(s.win_rate_90d), note: "Closed deals, last 90 days" },
    {
      label: "Time to win",
      value: s.avg_days_to_win === null ? "n/a" : `${Math.round(s.avg_days_to_win)} days`,
      note: "Average, last 180 days",
    },
  ];
  return (
    <dl className="rounded-control border-line bg-surface grid grid-cols-2 overflow-hidden border sm:grid-cols-3 lg:grid-cols-5">
      {figures.map((f) => (
        <div key={f.label} className="border-line p-4 not-last:border-b sm:border-r lg:border-b-0">
          <dt className="text-meta text-muted">{f.label}</dt>
          <dd className="text-title mt-1 font-semibold">{f.value}</dd>
          <dd className="text-meta text-muted">{f.note}</dd>
        </div>
      ))}
    </dl>
  );
}

function FunnelSection({ owner, from }: { owner: string; from: string }) {
  const funnel = useQuery({
    queryKey: keys.analytics("funnel", { owner, from }),
    queryFn: () => get<Funnel>(`/analytics/funnel${query({ owner_id: owner, from })}`),
  });
  return (
    <Section
      title="Where do deals drop off?"
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
      title="What could close in the next six months?"
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
      <Section title="Who is closing?" subtitle="Won value in the selected period, by deal owner.">
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
                  cell: (r) => <span className="font-medium">{r.full_name}</span>,
                },
                {
                  header: "Won",
                  cell: (r) => (
                    <div className="flex items-center gap-3">
                      <span className="num w-24 shrink-0 text-right">{fmt(r.won_cents)}</span>
                      <span className="hidden h-3 flex-1 sm:block" aria-hidden="true">
                        <span
                          className="bg-accent block h-full rounded-r-[4px]"
                          style={{ width: `${(r.won_cents / top) * 100}%` }}
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
