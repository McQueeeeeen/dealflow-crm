"use client";

import {
  DndContext,
  DragOverlay,
  MouseSensor,
  TouchSensor,
  closestCorners,
  useDroppable,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
  type UniqueIdentifier,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { IconGripVertical, IconSnowflake } from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type CSSProperties, type ReactNode } from "react";

import { DealLink, DealName } from "@/components/deal-link";
import { Avatar, cx } from "@/components/ui";
import { ApiError, post } from "@/lib/api";
import { parseDay, quietDays, shortDate } from "@/lib/format";
import { stageHeat } from "@/lib/heat";
import { useMoney } from "@/lib/hooks";
import { canModify, useInvalidate, useMe } from "@/lib/queries";
import type { BoardColumn, Deal } from "@/lib/types";

const GAP = 1024;

function positionBetween(prev?: Deal, next?: Deal): number {
  if (prev && next) return (prev.board_position + next.board_position) / 2;
  if (prev) return prev.board_position + GAP;
  if (next) return next.board_position - GAP;
  return 0;
}

interface Move {
  deal: Deal;
  stageId: string;
  position: number;
}

function planMove(
  columns: BoardColumn[],
  activeId: UniqueIdentifier,
  overId: UniqueIdentifier,
  overIsColumn: boolean,
): Move | null {
  const source = columns.find((c) => c.deals.some((d) => d.id === activeId));
  const target = overIsColumn
    ? columns.find((c) => c.stage.id === overId)
    : columns.find((c) => c.deals.some((d) => d.id === overId));
  const deal = source?.deals.find((d) => d.id === activeId);
  if (!source || !target || !deal) return null;

  let ordered: Deal[];
  let index: number;
  if (source === target) {
    const oldIndex = source.deals.findIndex((d) => d.id === activeId);
    const newIndex = overIsColumn
      ? source.deals.length - 1
      : source.deals.findIndex((d) => d.id === overId);
    if (oldIndex === newIndex) return null;
    ordered = arrayMove(source.deals, oldIndex, newIndex);
    index = newIndex;
  } else {
    const rest = target.deals.filter((d) => d.id !== activeId);
    index = overIsColumn
      ? rest.length
      : Math.max(
          0,
          rest.findIndex((d) => d.id === overId),
        );
    ordered = [...rest.slice(0, index), deal, ...rest.slice(index)];
  }
  return {
    deal,
    stageId: target.stage.id,
    position: positionBetween(ordered[index - 1], ordered[index + 1]),
  };
}

function applyMove(columns: BoardColumn[], move: Move): BoardColumn[] {
  const moved: Deal = { ...move.deal, stage_id: move.stageId, board_position: move.position };
  return columns.map((column) => {
    let deals = column.deals.filter((d) => d.id !== move.deal.id);
    if (column.stage.id === move.stageId) {
      moved.stage_name = column.stage.name;
      moved.stage_kind = column.stage.kind;
      deals = [...deals, moved].sort((a, b) => a.board_position - b.board_position);
    }
    return {
      ...column,
      deals,
      count: deals.length,
      total_value_cents: deals.reduce((sum, d) => sum + d.value_cents, 0),
    };
  });
}

export function Board({
  columns,
  queryKey,
}: {
  columns: BoardColumn[];
  queryKey: readonly unknown[];
}) {
  const client = useQueryClient();
  const invalidate = useInvalidate();
  const [active, setActive] = useState<Deal | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 6 } }),
    useSensor(TouchSensor, { activationConstraint: { delay: 220, tolerance: 8 } }),
  );

  const move = useMutation({
    mutationFn: (m: Move) =>
      post<Deal>(`/deals/${m.deal.id}/move`, {
        stage_id: m.stageId,
        board_position: m.position,
        version: m.deal.version,
      }),
    onMutate: async (m) => {
      setProblem(null);
      await client.cancelQueries({ queryKey });
      const previous = client.getQueryData<BoardColumn[]>(queryKey);
      if (previous) client.setQueryData(queryKey, applyMove(previous, m));
      return { previous };
    },
    onError: (err, m, context) => {
      if (context?.previous) client.setQueryData(queryKey, context.previous);
      setProblem(
        err instanceof ApiError && err.status === 409
          ? `“${m.deal.title}” was changed by someone else. The board has been refreshed, try the move again.`
          : err instanceof ApiError && err.status === 403
            ? `Only the owner or a manager can move “${m.deal.title}”.`
            : `Could not move “${m.deal.title}”. Check your connection and try again.`,
      );
    },
    onSuccess: (saved) => {
      client.setQueryData<BoardColumn[]>(queryKey, (cols) =>
        cols?.map((c) => ({ ...c, deals: c.deals.map((d) => (d.id === saved.id ? saved : d)) })),
      );
      client.setQueryData(["deal", saved.id], saved);
      setStatus(`${saved.title} moved to ${saved.stage_name}.`);
    },
    onSettled: (_data, err, m) => {
      document.querySelector<HTMLElement>(`[data-handle="${m.deal.id}"]`)?.focus();
      if (err) client.invalidateQueries({ queryKey });
      invalidate("deal");
    },
  });

  const dealById = (id: UniqueIdentifier) =>
    columns.flatMap((c) => c.deals).find((d) => d.id === id);
  const stages = columns.map((c) => c.stage);

  function step(deal: Deal, by: number) {
    const target = columns[columns.findIndex((c) => c.stage.id === deal.stage_id) + by];
    if (target) {
      move.mutate({
        deal,
        stageId: target.stage.id,
        position: positionBetween(undefined, target.deals[0]),
      });
    }
  }

  function onDragStart(event: DragStartEvent) {
    setActive(dealById(event.active.id) ?? null);
  }

  function onDragEnd({ active: dragged, over }: DragEndEvent) {
    setActive(null);
    if (!over) return;
    const plan = planMove(columns, dragged.id, over.id, over.data.current?.type === "column");
    if (plan) move.mutate(plan);
  }

  return (
    <>
      {problem ? (
        <p
          role="alert"
          className="rounded-control bg-critical-soft text-body text-critical mb-4 px-3 py-2"
        >
          {problem}
        </p>
      ) : null}
      <DndContext
        sensors={sensors}
        collisionDetection={closestCorners}
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
        onDragCancel={() => setActive(null)}
      >
        <div
          className={cx(
            "-mx-4 flex snap-x snap-mandatory items-start gap-3 overflow-x-auto px-4 pb-4 sm:-mx-6 sm:px-6",
            active && "select-none",
          )}
        >
          {columns.map((column) => (
            <Column
              key={column.stage.id}
              column={column}
              color={stageHeat(stages, column.stage)}
              onStep={step}
            />
          ))}
        </div>
        <DragOverlay dropAnimation={null}>
          {active ? <DealCard deal={active} floating /> : null}
        </DragOverlay>
      </DndContext>
      <p role="status" className="sr-only">
        {status}
      </p>
      <p id="board-keys" className="sr-only">
        Press the left or right arrow key to move this deal to the previous or next stage.
      </p>
    </>
  );
}

function Column({ column, color, onStep }: { column: BoardColumn; color: string; onStep: Step }) {
  const fmt = useMoney();
  const { setNodeRef, isOver } = useDroppable({ id: column.stage.id, data: { type: "column" } });
  const { stage } = column;
  return (
    <section
      aria-labelledby={`stage-${stage.id}`}
      style={{ "--heat": color } as CSSProperties}
      className={cx(
        "rounded-card bg-surface shadow-highlight flex w-[78vw] max-w-72 shrink-0 snap-start flex-col border sm:w-72",
        isOver ? "border-[var(--heat)]" : "border-line",
      )}
    >
      <header className="border-line border-b px-3.5 pt-3.5 pb-3">
        <h2 id={`stage-${stage.id}`} className="flex items-center justify-between gap-2">
          <span className="flex items-center gap-2 font-semibold">
            <span aria-hidden="true" className="size-2.5 rounded-full bg-[var(--heat)]" />
            {stage.name}
          </span>
          <span className="num text-meta text-muted">
            {stage.kind === "open" ? `${stage.probability}%` : null}
          </span>
        </h2>
        <p className="num mt-1 flex items-baseline justify-between gap-2">
          <span className="text-title font-semibold">
            {fmt(column.total_value_cents, { compact: true })}
          </span>
          <span className="text-meta text-muted">
            {column.count} {column.count === 1 ? "deal" : "deals"}
          </span>
        </p>
      </header>
      <SortableContext items={column.deals.map((d) => d.id)} strategy={verticalListSortingStrategy}>
        <ul
          ref={setNodeRef}
          className="flex min-h-24 flex-col gap-2 p-2 sm:max-h-[calc(100dvh-17rem)] sm:overflow-y-auto"
        >
          {column.deals.map((deal) => (
            <SortableDeal key={deal.id} deal={deal} onStep={onStep} />
          ))}
          {column.deals.length === 0 ? (
            <li className="text-meta text-muted px-2 py-6 text-center">Drop a deal here</li>
          ) : null}
        </ul>
      </SortableContext>
    </section>
  );
}

type Step = (deal: Deal, by: number) => void;

function SortableDeal({ deal, onStep }: { deal: Deal; onStep: Step }) {
  const me = useMe().data;
  const movable = canModify(me, deal.owner_id);
  const { listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: deal.id,
    disabled: !movable,
  });

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={cx(
        "[contain-intrinsic-size:auto_7rem] [content-visibility:auto]",
        isDragging && "opacity-40",
      )}
      inert={isDragging}
      {...listeners}
    >
      <DealCard
        deal={deal}
        handle={
          movable ? (
            <button
              type="button"
              data-handle={deal.id}
              aria-label={`Move ${deal.title}`}
              aria-describedby="board-keys"
              onKeyDown={(e) => {
                const by = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
                if (by) {
                  e.preventDefault();
                  onStep(deal, by);
                }
              }}
              className="rounded-control text-muted hover:bg-hover hover:text-ink -mt-1 -mr-1 inline-flex size-8 shrink-0 cursor-grab items-center justify-center"
            >
              <IconGripVertical size={16} stroke={1.75} aria-hidden="true" />
            </button>
          ) : null
        }
      />
    </li>
  );
}

function DealCard({
  deal,
  floating,
  handle,
}: {
  deal: Deal;
  floating?: boolean;
  handle?: ReactNode;
}) {
  const fmt = useMoney();
  const me = useMe().data;
  const quiet = quietDays(deal, me?.stale_after_days);
  const slipped =
    deal.stage_kind === "open" &&
    deal.expected_close_date !== null &&
    parseDay(deal.expected_close_date) < new Date(new Date().toDateString());
  return (
    <article
      className={cx(
        "border-line bg-row shadow-highlight rounded-xl border p-3.5",
        floating ? "shadow-float rotate-1 cursor-grabbing" : "hover:border-line-strong cursor-grab",
      )}
    >
      <div className="flex items-start justify-between gap-2">
        {floating ? (
          <span className="min-w-0 font-medium break-words">{deal.title}</span>
        ) : (
          <DealLink
            deal={deal}
            className="min-w-0 font-medium break-words hover:underline hover:underline-offset-2"
          >
            <DealName deal={deal} className="block" />
          </DealLink>
        )}
        {handle}
      </div>
      {deal.company_name ? (
        <p className="text-meta text-muted mt-0.5 truncate">{deal.company_name}</p>
      ) : null}
      <div className="mt-2 flex items-baseline justify-between gap-2">
        <span className="num font-medium">{fmt(deal.value_cents)}</span>
        <span className="text-meta text-muted flex items-baseline gap-2">
          {quiet !== null ? (
            <span
              className="text-open flex items-center gap-1 self-center font-medium"
              title={`No activity for ${quiet} days`}
            >
              <IconSnowflake size={14} stroke={2} aria-hidden="true" />
              <span className="num">{quiet}d</span>
              <span className="sr-only"> without activity</span>
            </span>
          ) : null}
          {deal.expected_close_date && deal.stage_kind === "open" ? (
            <span className={cx("num", slipped && "text-critical font-medium")}>
              {shortDate(deal.expected_close_date)}
            </span>
          ) : null}
          {deal.owner_id !== me?.id ? (
            <span title={deal.owner_name} className="self-center">
              <Avatar name={deal.owner_name} />
            </span>
          ) : null}
        </span>
      </div>
    </article>
  );
}
