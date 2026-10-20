"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useId, useState } from "react";

import { TaskFormDialog } from "@/components/task-form";
import { cx } from "@/components/ui";
import { del, post } from "@/lib/api";
import { isOverdue, relativeDue } from "@/lib/format";
import { canModify, useMe } from "@/lib/queries";
import type { Task } from "@/lib/types";
import { FORWARD } from "@/lib/transitions";

export function TaskRow({ task, showLinks = true }: { task: Task; showLinks?: boolean }) {
  const me = useMe().data;
  const client = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [intent, setIntent] = useState<boolean | null>(null);
  const checkId = useId();
  const done = !!task.completed_at;
  const overdue = isOverdue(task);
  const allowed = canModify(me, task.assignee_id) || task.created_by === me?.id;

  const toggle = useMutation({
    mutationFn: (complete: boolean) =>
      post<Task>(`/tasks/${task.id}/${complete ? "complete" : "reopen"}`),
    onError: () => setIntent(null),
    onSettled: () => {
      client.invalidateQueries({ queryKey: ["tasks"] });
      client.invalidateQueries({ queryKey: ["audit"] });
    },
  });
  const remove = useMutation({
    mutationFn: () => del(`/tasks/${task.id}`),
    onSuccess: () => client.invalidateQueries({ queryKey: ["tasks"] }),
  });

  const checked = intent ?? done;

  return (
    <li className="flex items-start gap-3 px-4 py-3">
      <input
        id={checkId}
        type="checkbox"
        checked={checked}
        disabled={!allowed || toggle.isPending}
        onChange={() => {
          setIntent(!checked);
          toggle.mutate(!checked);
        }}
        aria-label={done ? `Reopen “${task.title}”` : `Mark “${task.title}” done`}
        className="mt-0.5 size-5 shrink-0 accent-[var(--accent)]"
      />
      <div className="min-w-0 flex-1">
        <label
          htmlFor={checkId}
          className={cx("block cursor-pointer break-words", checked && "text-muted line-through")}
        >
          {task.title}
        </label>
        <p className="text-meta text-muted mt-0.5 flex flex-wrap gap-x-3 gap-y-0.5">
          {task.due_at ? (
            <span className={cx("num", overdue && "text-critical font-medium")}>
              {overdue ? "Overdue, " : ""}
              {relativeDue(task.due_at)}
            </span>
          ) : (
            <span>No due date</span>
          )}
          {showLinks && task.deal_id ? (
            <Link
              transitionTypes={FORWARD}
              href={`/deals/${task.deal_id}`}
              className="hover:text-ink underline underline-offset-2"
            >
              {task.deal_title}
            </Link>
          ) : null}
          {showLinks && task.contact_id ? (
            <Link
              transitionTypes={FORWARD}
              href={`/contacts/${task.contact_id}`}
              className="hover:text-ink underline underline-offset-2"
            >
              {task.contact_name}
            </Link>
          ) : null}
          {task.assignee_id !== me?.id ? <span>{task.assignee_name}</span> : null}
        </p>
        {toggle.isError || remove.isError ? (
          <p role="alert" className="text-meta text-critical mt-1">
            That change did not save. Try again.
          </p>
        ) : null}
      </div>
      {allowed ? (
        <div className="flex shrink-0 gap-1">
          <button
            type="button"
            onClick={() => setEditing(true)}
            className="rounded-control text-meta text-muted hover:bg-hover hover:text-ink min-h-11 px-2 sm:min-h-8"
          >
            Edit
          </button>
          <button
            type="button"
            onClick={() => {
              if (window.confirm(`Delete “${task.title}”?`)) remove.mutate();
            }}
            className="rounded-control text-meta text-muted hover:bg-critical-soft hover:text-critical min-h-11 px-2 sm:min-h-8"
          >
            Delete
          </button>
        </div>
      ) : null}
      <TaskFormDialog open={editing} onClose={() => setEditing(false)} task={task} />
    </li>
  );
}

export function TaskGroup({
  title,
  tasks,
  tone,
}: {
  title: string;
  tasks: Task[];
  tone?: "critical";
}) {
  if (tasks.length === 0) return null;
  return (
    <div>
      <h3
        className={cx(
          "border-line text-body flex items-baseline gap-2 border-b px-4 pt-4 pb-2 font-semibold",
          tone === "critical" && "text-critical",
        )}
      >
        {title}
        <span className="num text-meta text-muted font-normal">{tasks.length}</span>
      </h3>
      <ul className="divide-line divide-y">
        {tasks.map((task) => (
          <TaskRow key={task.id} task={task} />
        ))}
      </ul>
    </div>
  );
}
