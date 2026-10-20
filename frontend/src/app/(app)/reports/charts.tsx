"use client";

import { useState } from "react";

import { cx } from "@/components/ui";
import { monthLabel, percent } from "@/lib/format";
import { useMoney } from "@/lib/hooks";
import type { ForecastMonth, FunnelStage } from "@/lib/types";

/** Horizontal bars, one hue: how many deals from the cohort reached each stage. */
export function FunnelChart({ stages, cohort }: { stages: FunnelStage[]; cohort: number }) {
  const max = Math.max(cohort, 1);
  return (
    <ol className="flex flex-col gap-3">
      {stages.map((stage) => {
        const share = stage.reached / max;
        return (
          <li key={stage.stage_id} className="grid grid-cols-[7.5rem_1fr] items-center gap-3 sm:grid-cols-[9rem_1fr]">
            <span className="truncate text-body">{stage.name}</span>
            <div className="flex min-w-0 items-center gap-3">
              <div className="h-5 min-w-0 flex-1">
                <div
                  className={cx(
                    "h-full rounded-r-[4px]",
                    stage.kind === "won" ? "bg-accent" : "bg-accent/70",
                  )}
                  style={{ width: `${Math.max(share * 100, stage.reached > 0 ? 1.5 : 0)}%` }}
                />
              </div>
              <span className="num w-28 shrink-0 text-meta text-muted sm:w-36">
                <span className="font-medium text-ink">{stage.reached}</span>
                {stage.conversion_from_previous !== null
                  ? `, ${percent(stage.conversion_from_previous)} of previous`
                  : " deals"}
              </span>
            </div>
          </li>
        );
      })}
    </ol>
  );
}

/**
 * Columns per month. The light column is the full open value; the accent column inside it
 * is the same deals weighted by stage probability, so the gap between them is the risk.
 */
export function ForecastChart({ months }: { months: ForecastMonth[] }) {
  const fmt = useMoney();
  const [focus, setFocus] = useState<number | null>(null);
  const max = Math.max(...months.map((m) => m.total_cents), 1);
  const ticks = [0, 0.5, 1].map((t) => t * max);
  const active = focus !== null ? months[focus] : undefined;

  return (
    <div>
      <div className="mb-3 flex flex-wrap gap-x-5 gap-y-1 text-meta text-muted">
        <span className="flex items-center gap-2">
          <span aria-hidden="true" className="inline-block size-3 rounded-[2px] bg-track" />
          Full value of open deals
        </span>
        <span className="flex items-center gap-2">
          <span aria-hidden="true" className="inline-block size-3 rounded-[2px] bg-accent" />
          Weighted by stage probability
        </span>
      </div>

      <div className="relative">
        <div className="grid grid-cols-[3.5rem_1fr] gap-2">
          <div className="relative h-48" aria-hidden="true">
            {ticks.map((t) => (
              <span
                key={t}
                className="num absolute right-0 -translate-y-1/2 text-meta text-muted"
                style={{ bottom: `${(t / max) * 100}%` }}
              >
                {fmt(t, { compact: true })}
              </span>
            ))}
          </div>
          <div className="relative h-48 border-b border-line">
            {ticks.slice(1).map((t) => (
              <div
                key={t}
                aria-hidden="true"
                className="absolute inset-x-0 border-t border-line"
                style={{ bottom: `${(t / max) * 100}%` }}
              />
            ))}
            <div className="relative flex h-full items-end justify-around gap-1">
              {months.map((m, i) => (
                <button
                  key={m.month}
                  type="button"
                  className="group relative flex h-full flex-1 items-end justify-center rounded-control focus-visible:outline-offset-0"
                  onMouseEnter={() => setFocus(i)}
                  onMouseLeave={() => setFocus(null)}
                  onFocus={() => setFocus(i)}
                  onBlur={() => setFocus(null)}
                  aria-label={`${monthLabel(m.month)}: ${m.deals} deals, ${fmt(m.total_cents)} full value, ${fmt(m.weighted_cents)} weighted`}
                >
                  <span
                    className={cx("relative block w-full max-w-6 rounded-t-[4px] bg-track", focus === i && "brightness-95")}
                    style={{ height: `${(m.total_cents / max) * 100}%` }}
                  >
                    <span
                      className="absolute inset-x-0 bottom-0 block rounded-t-[4px] bg-accent"
                      style={{ height: m.total_cents > 0 ? `${(m.weighted_cents / m.total_cents) * 100}%` : 0 }}
                    />
                  </span>
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="mt-2 grid grid-cols-[3.5rem_1fr] gap-2" aria-hidden="true">
          <span />
          <div className="flex justify-around gap-1">
            {months.map((m) => (
              <span key={m.month} className="flex-1 text-center text-meta whitespace-nowrap text-muted">
                {monthLabel(m.month).split(" ")[0]}
              </span>
            ))}
          </div>
        </div>

        <p aria-live="polite" className="num mt-3 min-h-6 text-body">
          {active ? (
            <>
              <span className="font-medium">{monthLabel(active.month)}</span>
              <span className="text-muted">
                {": "}
                {active.deals} {active.deals === 1 ? "deal" : "deals"}, {fmt(active.total_cents)} full value,{" "}
              </span>
              <span className="font-medium">{fmt(active.weighted_cents)} weighted</span>
            </>
          ) : (
            <span className="text-muted">Hover or focus a month for its numbers.</span>
          )}
        </p>
      </div>

      <details className="mt-3">
        <summary className="cursor-pointer text-body text-accent">Show as a table</summary>
        <table className="mt-2 w-full text-body">
          <thead>
            <tr className="border-b border-line text-left text-meta text-muted">
              <th className="py-2 font-medium">Month</th>
              <th className="py-2 text-right font-medium">Deals</th>
              <th className="py-2 text-right font-medium">Full value</th>
              <th className="py-2 text-right font-medium">Weighted</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {months.map((m) => (
              <tr key={m.month} className="num">
                <td className="py-2">{monthLabel(m.month)}</td>
                <td className="py-2 text-right">{m.deals}</td>
                <td className="py-2 text-right">{fmt(m.total_cents)}</td>
                <td className="py-2 text-right">{fmt(m.weighted_cents)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  );
}
