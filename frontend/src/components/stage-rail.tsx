"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";

import { Button, cx } from "@/components/ui";
import { ApiError, post } from "@/lib/api";
import { shortDate } from "@/lib/format";
import { stageHeat } from "@/lib/heat";
import { keys, useStages } from "@/lib/queries";
import type { Deal } from "@/lib/types";

export function StageRail({ deal, editable }: { deal: Deal; editable: boolean }) {
  const stages = useStages().data ?? [];
  const client = useQueryClient();
  const open = stages.filter((s) => s.kind === "open");
  const won = stages.find((s) => s.kind === "won");
  const lost = stages.find((s) => s.kind === "lost");
  const closed = deal.stage_kind !== "open";
  const reached =
    deal.stage_kind === "won" ? open.length : open.findIndex((s) => s.id === deal.stage_id) + 1;

  const move = useMutation({
    mutationFn: (stageId: string) =>
      post<Deal>(`/deals/${deal.id}/move`, { stage_id: stageId, version: deal.version }),
    onSuccess: (saved) => {
      client.setQueryData(keys.deal(deal.id), saved);
      for (const key of [
        ["board"],
        ["deals"],
        ["analytics"],
        ["audit"],
        keys.stageHistory(deal.id),
      ]) {
        client.invalidateQueries({ queryKey: key });
      }
    },
    onError: (err) => {
      if (err instanceof ApiError && err.status === 409) {
        client.invalidateQueries({ queryKey: keys.deal(deal.id) });
      }
    },
  });

  const summary = closed
    ? `${deal.stage_kind === "won" ? "Won" : "Lost"}${deal.closed_at ? ` ${shortDate(deal.closed_at)}` : ""}.${editable ? " Pick a stage to reopen it." : ""}`
    : `${deal.probability}% likely to close.`;

  return (
    <section aria-label="Stage" className="border-line border-y py-4">
      <ol
        className="grid gap-3"
        style={{ gridTemplateColumns: `repeat(${open.length}, minmax(0, 1fr))` }}
      >
        {open.map((stage, i) => {
          const current = stage.id === deal.stage_id;
          const body = (
            <>
              <span aria-hidden="true" className="flex justify-between gap-[3px] overflow-hidden">
                {Array.from({ length: 14 }, (_, t) => (
                  <span
                    key={t}
                    className={cx(
                      "h-6 w-1.5 shrink-0 rounded-full",
                      (deal.stage_kind === "lost" || i >= reached) && "bg-track",
                    )}
                    style={
                      deal.stage_kind !== "lost" && i < reached
                        ? { backgroundColor: stageHeat(stages, stage) }
                        : undefined
                    }
                  />
                ))}
              </span>
              <span
                className={cx(
                  "text-body mt-2 block truncate",
                  current ? "text-ink font-semibold" : "text-muted",
                )}
              >
                {stage.name}
              </span>
              <span className="num text-meta text-muted block">{stage.probability}%</span>
            </>
          );
          return (
            <li key={stage.id}>
              {editable && !current ? (
                <button
                  type="button"
                  onClick={() => move.mutate(stage.id)}
                  disabled={move.isPending}
                  aria-label={`Move to ${stage.name}`}
                  className="rounded-control hover:bg-hover w-full p-1 text-left"
                >
                  {body}
                </button>
              ) : (
                <div aria-current={current ? "step" : undefined} className="p-1">
                  {body}
                </div>
              )}
            </li>
          );
        })}
      </ol>
      <div className="mt-3 flex flex-wrap items-center gap-3 px-1">
        <p
          className={cx(
            "text-body",
            deal.stage_kind === "won" && "text-won font-medium",
            deal.stage_kind === "lost" && "text-critical font-medium",
            !closed && "text-muted",
          )}
        >
          {summary}
        </p>
        {editable && !closed && won && lost ? (
          <div className="flex gap-2 sm:ml-auto">
            <Button variant="primary" onClick={() => move.mutate(won.id)} disabled={move.isPending}>
              Mark as Won
            </Button>
            <Button variant="danger" onClick={() => move.mutate(lost.id)} disabled={move.isPending}>
              Mark as Lost
            </Button>
          </div>
        ) : null}
      </div>
      {move.isError ? (
        <p role="alert" className="text-body text-critical mt-2 px-1">
          {move.error instanceof ApiError && move.error.status === 409
            ? "Someone changed this deal a moment ago. The page has been refreshed, try again."
            : "The move did not save. Try again."}
        </p>
      ) : null}
    </section>
  );
}
