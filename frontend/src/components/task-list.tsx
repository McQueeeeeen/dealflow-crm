"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useState } from "react";

import { TaskFormDialog } from "@/components/task-form";
import { cx } from "@/components/ui";
import { del, post } from "@/lib/api";
import { isOverdue, relativeDue } from "@/lib/format";
import { canModify, useMe } from "@/lib/queries";
import type { Task } from "@/lib/types";

export function TaskRow({ task, showLinks = true }: { task: Task; showLinks?: boolean }) {
  const me = useMe().data;
  const client = useQueryClient();
  const [editing, setEditing] = useState(false);
  const done = !!task.completed_at;
  const overdue = isOverdue(task);
  const allowed = canModify(me, task.assignee_id) || task.created_by === me?.id;

  const toggle = useMutation({
    mutationFn: () => post<Task>(`/tasks/${task.id}/${done ? "reopen" : "complete"}`),
    onSettled: () => {
      client.invalidateQueries({ queryKey: ["tasks"] });
      client.invalidateQueries({ queryKey: ["audit"] });
    },
  });
  const remove = useMutation({
    mutationFn: () => del(`/tasks/${task.id}`),
    onSuccess: () => client.invalidateQueries({ queryKey: ["tasks"] }),
  });

  // Show the intended state right away; the list refetches when the request settles.
  const checked = toggle.isPending ? !done : done;

  return (
    <li className="flex items-start gap-3 px-4 py-3">
      <input
        type="checkbox"
        checked={checked}
        disabled={!allowed || toggle.isPending}
        onChange={() => toggle.mutate()}
        aria-label={done ? `Reopen "${task.title}"` : `Mark "${task.title}" done`}
        className="mt-0.5 size-5 shrink-0 accent-[var(--accent)]"
      />
      <div className="min-w-0 flex-1">
        <p className={cx("break-words", checked && "text-muted line-through")}>{task.title}</p>
        <p className="mt-0.5 flex flex-wrap gap-x-3 gap-y-0.5 text-meta text-muted">
          {task.due_at ? (
            <span className={cx("num", overdue && "font-medium text-critical")}>
              {overdue ? "Overdue, " : ""}
              {relativeDue(task.due_at)}
            </span>
          ) : (
            <span>No due date</span>
          )}
          {showLinks && task.deal_id ? (
            <Link href={`/deals/${task.deal_id}`} className="underline underline-offset-2 hover:text-ink">
              {task.deal_title}
            </Link>
          ) : null}
          {showLinks && task.contact_id ? (
            <Link href={`/contacts/${task.contact_id}`} className="underline underline-offset-2 hover:text-ink">
              {task.contact_name}
            </Link>
          ) : null}
          {task.assignee_id !== me?.id ? <span>{task.assignee_name}</span> : null}
        </p>
        {toggle.isError || remove.isError ? (
          <p role="alert" className="mt-1 text-meta text-critical">
            That change did not save. Try again.
          </p>
        ) : null}
      </div>
      {allowed ? (
        <div className="flex shrink-0 gap-1">
          <button
            type="button"
            onClick={() => setEditing(true)}
            className="min-h-11 rounded-control px-2 text-meta text-muted hover:bg-hover hover:text-ink sm:min-h-8"
          >
            Edit
          </button>
          <button
            type="button"
            onClick={() => {
              if (window.confirm(`Delete "${task.title}"?`)) remove.mutate();
            }}
            className="min-h-11 rounded-control px-2 text-meta text-muted hover:bg-critical-soft hover:text-critical sm:min-h-8"
          >
            Delete
          </button>
        </div>
      ) : null}
      <TaskFormDialog open={editing} onClose={() => setEditing(false)} task={task} />
    </li>
  );
}

export function TaskGroup({ title, tasks, tone }: { title: string; tasks: Task[]; tone?: "critical" }) {
  if (tasks.length === 0) return null;
  return (
    <div>
      <h3
        className={cx(
          "flex items-baseline gap-2 border-b border-line px-4 pt-4 pb-2 text-body font-semibold",
          tone === "critical" && "text-critical",
        )}
      >
        {title}
        <span className="num text-meta font-normal text-muted">{tasks.length}</span>
      </h3>
      <ul className="divide-y divide-line">
        {tasks.map((task) => (
          <TaskRow key={task.id} task={task} />
        ))}
      </ul>
    </div>
  );
}
