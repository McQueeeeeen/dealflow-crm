"use client";

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { DealFormDialog } from "@/components/deal-form";
import { OwnerFilter } from "@/components/owner-select";
import { Button, EmptyState, ErrorState, Loading, PageHeader, TextInput } from "@/components/ui";
import { get, query } from "@/lib/api";
import { useDebounced, useMoney } from "@/lib/hooks";
import { keys } from "@/lib/queries";
import type { BoardColumn } from "@/lib/types";

import { Board } from "./board";

export default function PipelinePage() {
  const fmt = useMoney();
  const [search, setSearch] = useState("");
  const [owner, setOwner] = useState("");
  const [creating, setCreating] = useState(false);
  const q = useDebounced(search.trim());
  const queryKey = keys.board({ q, owner });

  const board = useQuery({
    queryKey,
    queryFn: () => get<BoardColumn[]>(`/deals/board${query({ q, owner_id: owner })}`),
  });

  const open = board.data?.filter((c) => c.stage.kind === "open") ?? [];
  const openValue = open.reduce((sum, c) => sum + c.total_value_cents, 0);
  const openCount = open.reduce((sum, c) => sum + c.count, 0);
  const total = board.data?.reduce((sum, c) => sum + c.count, 0) ?? 0;
  const filtered = q !== "" || owner !== "";

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
          <Button variant="primary" onClick={() => setCreating(true)}>
            New deal
          </Button>
        }
      />
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <label className="flex min-w-0 flex-1 items-center gap-2 sm:max-w-sm">
          <span className="sr-only">Search deals</span>
          <TextInput
            type="search"
            placeholder="Search deals by name"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </label>
        <OwnerFilter value={owner} onChange={setOwner} />
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
              Create the first deal
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
