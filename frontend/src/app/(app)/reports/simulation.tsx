"use client";

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { ErrorState, Loading, Panel, TickMeter, cx } from "@/components/ui";
import { get, query } from "@/lib/api";
import { percent, shortDate } from "@/lib/format";
import { heat } from "@/lib/heat";
import { useMoney } from "@/lib/hooks";
import { keys, useStages } from "@/lib/queries";
import type { Simulation, SimulationPeriod } from "@/lib/types";

const LABELS = { month: "This month", quarter: "This quarter" } as const;

export function SimulationSection({ owner }: { owner: string }) {
  const [key, setKey] = useState<SimulationPeriod["key"]>("month");
  const simulation = useQuery({
    queryKey: keys.analytics("simulation", { owner }),
    queryFn: () => get<Simulation>(`/analytics/simulation${query({ owner_id: owner })}`),
  });
  const period = simulation.data?.periods.find((p) => p.key === key);

  return (
    <Panel className="mt-6 p-4 sm:p-5">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="max-w-2xl">
          <h2 className="text-title font-semibold">What Will Actually Close?</h2>
          <p className="text-body text-muted mt-1">
            {simulation.data
              ? `${simulation.data.runs.toLocaleString()} simulated futures for ${simulation.data.open_deals} open deals, `
              : "Thousands of simulated futures for the open deals, "}
            using how often each stage really advanced and how long deals sat in it over the last
            year.
          </p>
        </div>
        <div role="group" aria-label="Period" className="bg-row flex rounded-lg p-1">
          {(["month", "quarter"] as const).map((k) => (
            <button
              key={k}
              type="button"
              aria-pressed={key === k}
              onClick={() => setKey(k)}
              className={cx(
                "text-body min-h-9 rounded-md px-3 font-medium",
                key === k ? "bg-surface text-ink shadow-highlight" : "text-muted hover:text-ink",
              )}
            >
              {LABELS[k]}
            </button>
          ))}
        </div>
      </div>
      {simulation.isPending ? (
        <Loading label="Running the simulation…" />
      ) : simulation.isError ? (
        <ErrorState error={simulation.error} onRetry={() => simulation.refetch()} />
      ) : period ? (
        <div className="mt-6 grid gap-8 xl:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
          <Outcome period={period} />
          <Model simulation={simulation.data} />
        </div>
      ) : null}
    </Panel>
  );
}

function Outcome({ period }: { period: SimulationPeriod }) {
  const fmt = useMoney();
  const d = period.distribution;
  const bins = d.histogram;
  const peak = Math.max(...bins.map((b) => b.runs), 1);
  const low = bins[0]?.from_cents ?? 0;
  const high = bins.at(-1)?.to_cents ?? 1;
  const at = (cents: number) => `${((cents - low) / Math.max(high - low, 1)) * 100}%`;
  const target = period.target_cents;

  return (
    <div className="min-w-0">
      <div className="flex flex-wrap items-end gap-x-8 gap-y-3">
        <div>
          <p className="text-meta text-muted">Most likely by {shortDate(period.ends_on)}</p>
          <p className="num text-hero font-semibold tracking-tight">{fmt(d.p50_cents)}</p>
        </div>
        <p className="text-body text-muted max-w-sm pb-2">
          8 in 10 futures land between{" "}
          <span className="num text-ink font-medium">{fmt(d.p10_cents)}</span> and{" "}
          <span className="num text-ink font-medium">{fmt(d.p90_cents)}</span>
          {period.won_cents > 0 ? (
            <>
              , including <span className="num text-won">{fmt(period.won_cents)}</span> already won
            </>
          ) : null}
          .
        </p>
      </div>

      <div
        role="img"
        aria-label={`Distribution of simulated revenue from ${fmt(low)} to ${fmt(high)}. Median ${fmt(d.p50_cents)}.`}
        className="relative mt-6 h-44"
      >
        <div className="flex h-full items-end gap-[3px]">
          {bins.map((bin) => {
            const inside = bin.to_cents > d.p10_cents && bin.from_cents <= d.p90_cents;
            return (
              <span
                key={bin.from_cents}
                className={cx(
                  "min-h-[3px] flex-1 rounded-t-[4px]",
                  inside ? "bg-accent" : "hatch border-line border border-b-0",
                )}
                style={{ height: `${(bin.runs / peak) * 100}%` }}
              />
            );
          })}
        </div>
        <Marker left={at(d.p50_cents)} label="Median" />
        {target !== null && target >= low && target <= high ? (
          <Marker left={at(target)} label="Target" tone="won" />
        ) : null}
      </div>
      <div className="num text-meta text-muted mt-2 flex justify-between">
        <span>{fmt(low, { compact: true })}</span>
        <span>{fmt(high, { compact: true })}</span>
      </div>

      {target !== null && period.chance_to_hit_target !== null ? (
        <div className="bg-row mt-6 flex flex-col gap-3 rounded-xl p-4">
          <p className="text-body">
            <span className="num text-won text-title font-semibold">
              {percent(period.chance_to_hit_target)}
            </span>{" "}
            <span className="text-muted">
              chance to reach the {fmt(target)} target for {LABELS[period.key].toLowerCase()}.
            </span>
          </p>
          <TickMeter
            label={`${percent(period.chance_to_hit_target)} chance to reach the target`}
            value={period.chance_to_hit_target}
            color="var(--won)"
            ticks={60}
          />
        </div>
      ) : null}
    </div>
  );
}

function Marker({ left, label, tone }: { left: string; label: string; tone?: "won" }) {
  return (
    <span
      aria-hidden="true"
      className={cx(
        "pointer-events-none absolute inset-y-0 border-l border-dashed",
        tone === "won" ? "border-won" : "border-ink",
      )}
      style={{ left }}
    >
      <span
        className={cx(
          "text-meta bg-surface absolute -top-1 left-1.5 rounded px-1 font-medium whitespace-nowrap",
          tone === "won" ? "text-won" : "text-ink",
        )}
      >
        {label}
      </span>
    </span>
  );
}

function Model({ simulation }: { simulation: Simulation }) {
  const stages = useStages().data ?? [];
  const open = stages.filter((s) => s.kind === "open");
  return (
    <div className="min-w-0">
      <h3 className="text-body font-semibold">What the Model Learned</h3>
      <p className="text-meta text-muted mt-1">
        Stages with little history lean on their configured probability until real moves outweigh
        it.
      </p>
      <table className="text-body mt-3 w-full border-separate border-spacing-y-1.5">
        <thead>
          <tr className="text-meta text-muted text-left">
            <th className="px-3 font-medium">Stage</th>
            <th className="px-3 text-right font-medium">Moves on</th>
            <th className="px-3 text-right font-medium">Typical stay</th>
            <th className="hidden px-3 text-right font-medium sm:table-cell">Seen</th>
          </tr>
        </thead>
        <tbody>
          {simulation.stages.map((s) => {
            const i = open.findIndex((o) => o.id === s.stage_id);
            return (
              <tr key={s.stage_id} className="num">
                <td className="bg-row rounded-l-lg px-3 py-2.5">
                  <span className="flex items-center gap-2 font-medium">
                    <span
                      aria-hidden="true"
                      className="size-2 rounded-full"
                      style={{ backgroundColor: heat(i / Math.max(open.length - 1, 1)) }}
                    />
                    {s.name}
                  </span>
                </td>
                <td className="bg-row px-3 py-2.5 text-right">{percent(s.advance_rate)}</td>
                <td className="bg-row px-3 py-2.5 text-right max-sm:rounded-r-lg">
                  {Math.round(s.median_days)} days
                </td>
                <td className="bg-row text-muted hidden rounded-r-lg px-3 py-2.5 text-right sm:table-cell">
                  {s.samples}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
