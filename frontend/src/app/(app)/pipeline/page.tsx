"use client";

import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";

import { DealFormDialog } from "@/components/deal-form";
import { FilterBar } from "@/components/filter-bar";
import { OwnerFilter } from "@/components/owner-select";
import { ExportLink } from "@/components/transfer";
import { Button, EmptyState, ErrorState, Loading, PageHeader } from "@/components/ui";
import { get, query } from "@/lib/api";
import { useMoney, useUrlParams } from "@/lib/hooks";
import { keys } from "@/lib/queries";
import type { BoardColumn } from "@/lib/types";

import { Board } from "./board";

export default function PipelinePage() {
  const fmt = useMoney();
  const [params, update] = useUrlParams();
  const filter = params.get("filter") ?? "";
  const owner = params.get("owner") ?? "";
  const applyFilter = useCallback((value: string) => update({ filter: value }), [update]);
  const [creating, setCreating] = useState(false);
  const queryKey = keys.board({ filter, owner });

  const board = useQuery({
    queryKey,
    queryFn: () => get<BoardColumn[]>(`/deals/board${query({ filter, owner_id: owner })}`),
  });

  const open = board.data?.filter((c) => c.stage.kind === "open") ?? [];
  const openValue = open.reduce((sum, c) => sum + c.total_value_cents, 0);
  const openCount = open.reduce((sum, c) => sum + c.count, 0);
  const total = board.data?.reduce((sum, c) => sum + c.count, 0) ?? 0;
  const filtered = filter !== "" || owner !== "";

  return (
    <>
      <PageHeader
        title="Pipeline"
        description={
          board.data ? (
            <span className="num">
              {fmt(openValue)} open across {openCount} {openCount === 1 ? "deal" : "deals"}
            </span>
          ) : undefined
        }
        actions={
          <>
            <ExportLink kind="deals" />
            <Button variant="primary" onClick={() => setCreating(true)}>
              New Deal
            </Button>
          </>
        }
      />
      <div className="flex flex-col gap-x-3 lg:flex-row lg:items-start">
        <div className="min-w-0 flex-1">
          <FilterBar filter={filter} onApply={applyFilter} />
        </div>
        <div className="mb-4">
          <OwnerFilter value={owner} onChange={(v) => update({ owner: v })} />
        </div>
      </div>

      {board.isPending ? (
        <Loading label="Loading the pipeline…" />
      ) : board.isError ? (
        <ErrorState error={board.error} onRetry={() => board.refetch()} />
      ) : total === 0 && !filtered ? (
        <EmptyState
          title="Your pipeline is empty"
          body="Create a deal for each opportunity you are working on. Drag it across stages as it moves forward."
          action={
            <Button variant="primary" onClick={() => setCreating(true)}>
              Create the First Deal
            </Button>
          }
        />
      ) : (
        <>
          {total === 0 ? (
            <p className="text-body text-muted mb-4">No deals match these filters.</p>
          ) : null}
          <Board columns={board.data} queryKey={queryKey} />
        </>
      )}

      <DealFormDialog open={creating} onClose={() => setCreating(false)} />
    </>
  );
}
