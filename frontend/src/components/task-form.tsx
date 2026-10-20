"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";

import {
  Button,
  Dialog,
  Field,
  FormError,
  Select,
  TextArea,
  TextInput,
  fieldError,
} from "@/components/ui";
import { post, put } from "@/lib/api";
import { useMe, useUsers } from "@/lib/queries";
import type { Task } from "@/lib/types";

interface Props {
  open: boolean;
  onClose: () => void;
  task?: Task;
  link?: { dealId?: string; contactId?: string };
}

export function TaskFormDialog(props: Props) {
  return (
    <Dialog
      open={props.open}
      onClose={props.onClose}
      title={props.task ? "Edit follow-up" : "New follow-up"}
    >
      {props.open ? <TaskForm {...props} /> : null}
    </Dialog>
  );
}

/** `datetime-local` works in local time without a zone, so convert at the edges. */
function toLocalInput(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function tomorrowMorning(): string {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  d.setHours(9, 0, 0, 0);
  return toLocalInput(d.toISOString());
}

function TaskForm({ task, link, onClose }: Props) {
  const me = useMe().data;
  const users = useUsers().data ?? [];
  const client = useQueryClient();
  const [values, setValues] = useState({
    title: task?.title ?? "",
    details: task?.details ?? "",
    due_at: task ? toLocalInput(task.due_at) : tomorrowMorning(),
    assignee_id: task?.assignee_id ?? me?.id ?? "",
  });

  const save = useMutation({
    mutationFn: (body: Record<string, unknown>) =>
      task ? put<Task>(`/tasks/${task.id}`, body) : post<Task>("/tasks", body),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: ["tasks"] });
      client.invalidateQueries({ queryKey: ["audit"] });
      onClose();
    },
  });

  function submit(e: FormEvent) {
    e.preventDefault();
    save.mutate({
      title: values.title,
      details: values.details || null,
      due_at: values.due_at ? new Date(values.due_at).toISOString() : null,
      assignee_id: values.assignee_id || null,
      deal_id: task ? task.deal_id : (link?.dealId ?? null),
      contact_id: task ? task.contact_id : (link?.contactId ?? null),
    });
  }

  const error = save.error;
  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-4">
      <Field label="What needs to happen" error={fieldError(error, "title")}>
        {(p) => (
          <TextInput
            {...p}
            value={values.title}
            onChange={(e) => setValues((v) => ({ ...v, title: e.target.value }))}
            placeholder="Send the revised proposal"
            autoFocus
          />
        )}
      </Field>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Due" error={fieldError(error, "due_at")}>
          {(p) => (
            <TextInput
              {...p}
              type="datetime-local"
              value={values.due_at}
              onChange={(e) => setValues((v) => ({ ...v, due_at: e.target.value }))}
            />
          )}
        </Field>
        <Field label="Assigned to" error={fieldError(error, "assignee_id")}>
          {(p) => (
            <Select
              {...p}
              value={values.assignee_id}
              onChange={(e) => setValues((v) => ({ ...v, assignee_id: e.target.value }))}
            >
              {users
                .filter((u) => u.is_active)
                .map((u) => (
                  <option key={u.id} value={u.id}>
                    {u.id === me?.id ? `${u.full_name} (you)` : u.full_name}
                  </option>
                ))}
            </Select>
          )}
        </Field>
      </div>
      <Field label="Details" error={fieldError(error, "details")}>
        {(p) => (
          <TextArea
            {...p}
            value={values.details}
            onChange={(e) => setValues((v) => ({ ...v, details: e.target.value }))}
          />
        )}
      </Field>
      <FormError error={error} />
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button type="button" variant="quiet" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" busy={save.isPending}>
          {task ? "Save follow-up" : "Add follow-up"}
        </Button>
      </div>
    </form>
  );
}
