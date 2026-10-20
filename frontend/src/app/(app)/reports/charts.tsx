"use client";

import { useState } from "react";

import { cx } from "@/components/ui";
import { monthLabel, percent } from "@/lib/format";
import { WON_HEAT, heat } from "@/lib/heat";
import { useMoney } from "@/lib/hooks";
import type { ForecastMonth, FunnelStage } from "@/lib/types";

export function FunnelChart({ stages, cohort }: { stages: FunnelStage[]; cohort: number }) {
  const max = Math.max(cohort, 1);
  return (
    <ol className="flex flex-col gap-3">
      {stages.map((stage, i) => {
        const share = stage.reached / max;
        return (
          <li
            key={stage.stage_id}
            className="grid grid-cols-[7.5rem_1fr] items-center gap-3 sm:grid-cols-[9rem_1fr]"
          >
            <span className="text-body truncate">{stage.name}</span>
            <div className="flex min-w-0 items-center gap-3">
              <div className="h-5 min-w-0 flex-1">
                <div
                  className="h-full rounded-full"
                  style={{
                    width: `${Math.max(share * 100, stage.reached > 0 ? 1.5 : 0)}%`,
                    backgroundColor:
                      stage.kind === "won" ? WON_HEAT : heat(i / Math.max(stages.length - 2, 1)),
                  }}
                />
              </div>
              <span className="num text-meta text-muted w-28 shrink-0 sm:w-36">
                <span className="text-ink font-medium">{stage.reached}</span>
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

export function ForecastChart({ months }: { months: ForecastMonth[] }) {
  const fmt = useMoney();
  const [focus, setFocus] = useState<number | null>(null);
  const max = Math.max(...months.map((m) => m.total_cents), 1);
  const ticks = [0, 0.5, 1].map((t) => t * max);
  const active = focus !== null ? months[focus] : undefined;

  return (
    <div>
      <div className="text-meta text-muted mb-6 flex flex-wrap gap-x-5 gap-y-1">
        <span className="flex items-center gap-2">
          <span
            aria-hidden="true"
            className="hatch border-line inline-block size-3 rounded-sm border"
          />
          Full value of open deals
        </span>
        <span className="flex items-center gap-2">
          <span aria-hidden="true" className="bg-accent inline-block size-3 rounded-sm" />
          Weighted by stage probability
        </span>
      </div>

      <div className="relative">
        <div className="grid grid-cols-[3.5rem_1fr] gap-2">
          <div className="relative h-48" aria-hidden="true">
            {ticks.map((t) => (
              <span
                key={t}
                className="num text-meta text-muted absolute right-0 -translate-y-1/2"
                style={{ bottom: `${(t / max) * 100}%` }}
              >
                {fmt(t, { compact: true })}
              </span>
            ))}
          </div>
          <div className="border-line relative h-48 border-b">
            {ticks.slice(1).map((t) => (
              <div
                key={t}
                aria-hidden="true"
                className="border-line absolute inset-x-0 border-t"
                style={{ bottom: `${(t / max) * 100}%` }}
              />
            ))}
            <div className="relative flex h-full items-end justify-around gap-1">
              {months.map((m, i) => (
                <button
                  key={m.month}
                  type="button"
                  className="group rounded-control relative flex h-full flex-1 items-end justify-center focus-visible:outline-offset-0"
                  onMouseEnter={() => setFocus(i)}
                  onMouseLeave={() => setFocus(null)}
                  onFocus={() => setFocus(i)}
                  onBlur={() => setFocus(null)}
                  aria-label={`${monthLabel(m.month)}: ${m.deals} deals, ${fmt(m.total_cents)} full value, ${fmt(m.weighted_cents)} weighted`}
                >
                  <span
                    className={cx(
                      "hatch border-line relative block w-full max-w-10 rounded-t-lg border border-b-0",
                      focus === i && "brightness-95",
                    )}
                    style={{ height: `${(m.total_cents / max) * 100}%` }}
                  >
                    <span
                      className="bg-accent absolute -inset-x-px bottom-0 block rounded-t-lg"
                      style={{
                        height:
                          m.total_cents > 0 ? `${(m.weighted_cents / m.total_cents) * 100}%` : 0,
                      }}
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
              <span
                key={m.month}
                className="text-meta text-muted flex-1 text-center whitespace-nowrap"
              >
                {monthLabel(m.month).split(" ")[0]}
              </span>
            ))}
          </div>
        </div>

        <p aria-live="polite" className="num text-body mt-3 min-h-6">
          {active ? (
            <>
              <span className="font-medium">{monthLabel(active.month)}</span>
              <span className="text-muted">
                {": "}
                {active.deals} {active.deals === 1 ? "deal" : "deals"}, {fmt(active.total_cents)}{" "}
                full value,{" "}
              </span>
              <span className="font-medium">{fmt(active.weighted_cents)} weighted</span>
            </>
          ) : (
            <span className="text-muted">Hover or focus a month for its numbers.</span>
          )}
        </p>
      </div>

      <details className="mt-3">
        <summary className="text-body text-link cursor-pointer">Show as a table</summary>
        <table className="text-body mt-2 w-full">
          <thead>
            <tr className="border-line text-meta text-muted border-b text-left">
              <th className="py-2 font-medium">Month</th>
              <th className="py-2 text-right font-medium">Deals</th>
              <th className="py-2 text-right font-medium">Full value</th>
              <th className="py-2 text-right font-medium">Weighted</th>
            </tr>
          </thead>
          <tbody className="divide-line divide-y">
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
