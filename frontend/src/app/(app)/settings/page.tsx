"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";

import { Table } from "@/components/table";
import {
  Button,
  Dialog,
  ErrorState,
  Field,
  FormError,
  Loading,
  PageHeader,
  Panel,
  Select,
  TextInput,
  cx,
  fieldError,
} from "@/components/ui";
import { ApiError, del, post, put } from "@/lib/api";
import { ROLE_LABELS, shortDate } from "@/lib/format";
import { keys, useMe, useStages, useUsers } from "@/lib/queries";
import type { Role, Stage, StageKind, User } from "@/lib/types";

export default function SettingsPage() {
  const me = useMe().data;
  if (me && me.role !== "admin") {
    return (
      <>
        <PageHeader title="Settings" />
        <p className="text-muted">Only workspace admins can manage the team and the pipeline.</p>
      </>
    );
  }
  return (
    <>
      <PageHeader title="Settings" description={me?.organization_name} />
      <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <TeamSection />
        <PipelineSection />
      </div>
    </>
  );
}

function TeamSection() {
  const me = useMe().data;
  const users = useUsers();
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<User | null>(null);

  return (
    <Panel>
      <div className="flex items-center justify-between gap-3 p-4">
        <div>
          <h2 className="text-title font-semibold">Team</h2>
          <p className="text-body text-muted">
            Sales reps edit their own records. Managers edit everything and see team reports.
          </p>
        </div>
        <Button variant="primary" onClick={() => setAdding(true)}>
          Add teammate
        </Button>
      </div>
      {users.isPending ? (
        <Loading label="Loading team…" />
      ) : users.isError ? (
        <ErrorState error={users.error} onRetry={() => users.refetch()} />
      ) : (
        <Table
          caption="Team members"
          rows={users.data}
          columns={[
            {
              header: "Name",
              cell: (u) => (
                <div className={cx(!u.is_active && "text-muted")}>
                  <p className="font-medium">
                    {u.full_name}
                    {u.id === me?.id ? (
                      <span className="text-muted font-normal"> (you)</span>
                    ) : null}
                  </p>
                  <p className="text-meta text-muted break-all">{u.email}</p>
                </div>
              ),
            },
            { header: "Role", cell: (u) => (u.is_active ? ROLE_LABELS[u.role] : "Deactivated") },
            {
              header: "Added",
              cell: (u) => <span className="num">{shortDate(u.created_at)}</span>,
              hideBelow: "md",
            },
            {
              header: "Actions",
              align: "right",
              cell: (u) =>
                u.id === me?.id ? null : (
                  <Button variant="quiet" onClick={() => setEditing(u)}>
                    Edit
                  </Button>
                ),
            },
          ]}
        />
      )}
      <Dialog open={adding} onClose={() => setAdding(false)} title="Add a teammate">
        {adding ? <AddUserForm onClose={() => setAdding(false)} /> : null}
      </Dialog>
      <Dialog open={editing !== null} onClose={() => setEditing(null)} title="Edit teammate">
        {editing ? <EditUserForm user={editing} onClose={() => setEditing(null)} /> : null}
      </Dialog>
    </Panel>
  );
}

function RoleSelect(props: { id: string; value: Role; onChange: (role: Role) => void }) {
  return (
    <Select
      id={props.id}
      value={props.value}
      onChange={(e) => props.onChange(e.target.value as Role)}
    >
      <option value="sales_rep">Sales rep</option>
      <option value="manager">Manager</option>
      <option value="admin">Admin</option>
    </Select>
  );
}

function AddUserForm({ onClose }: { onClose: () => void }) {
  const client = useQueryClient();
  const [values, setValues] = useState({
    full_name: "",
    email: "",
    role: "sales_rep" as Role,
    password: "",
  });
  const save = useMutation({
    mutationFn: () => post<User>("/users", values),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: keys.users });
      onClose();
    },
  });
  const error = save.error;
  return (
    <form
      noValidate
      className="flex flex-col gap-4"
      onSubmit={(e: FormEvent) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <Field label="Full name" error={fieldError(error, "full_name")}>
        {(p) => (
          <TextInput
            {...p}
            value={values.full_name}
            onChange={(e) => setValues((v) => ({ ...v, full_name: e.target.value }))}
            autoFocus
          />
        )}
      </Field>
      <Field label="Work email" error={fieldError(error, "email")}>
        {(p) => (
          <TextInput
            {...p}
            type="email"
            value={values.email}
            onChange={(e) => setValues((v) => ({ ...v, email: e.target.value }))}
          />
        )}
      </Field>
      <Field label="Role">
        {(p) => (
          <RoleSelect
            id={p.id}
            value={values.role}
            onChange={(role) => setValues((v) => ({ ...v, role }))}
          />
        )}
      </Field>
      <Field
        label="Initial password"
        error={fieldError(error, "password")}
        hint="At least 10 characters. Share it with them directly."
      >
        {(p) => (
          <TextInput
            {...p}
            type="text"
            autoComplete="off"
            value={values.password}
            onChange={(e) => setValues((v) => ({ ...v, password: e.target.value }))}
          />
        )}
      </Field>
      <FormError error={error} />
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button type="button" variant="quiet" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" busy={save.isPending}>
          Add teammate
        </Button>
      </div>
    </form>
  );
}

function EditUserForm({ user, onClose }: { user: User; onClose: () => void }) {
  const client = useQueryClient();
  const [values, setValues] = useState({
    full_name: user.full_name,
    role: user.role,
    is_active: user.is_active,
  });
  const save = useMutation({
    mutationFn: () => put<User>(`/users/${user.id}`, values),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: keys.users });
      onClose();
    },
  });
  const error = save.error;
  return (
    <form
      noValidate
      className="flex flex-col gap-4"
      onSubmit={(e: FormEvent) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <Field label="Full name" error={fieldError(error, "full_name")}>
        {(p) => (
          <TextInput
            {...p}
            value={values.full_name}
            onChange={(e) => setValues((v) => ({ ...v, full_name: e.target.value }))}
          />
        )}
      </Field>
      <Field label="Role">
        {(p) => (
          <RoleSelect
            id={p.id}
            value={values.role}
            onChange={(role) => setValues((v) => ({ ...v, role }))}
          />
        )}
      </Field>
      <label className="flex min-h-11 items-center gap-3">
        <input
          type="checkbox"
          className="size-5 accent-[var(--accent)]"
          checked={values.is_active}
          onChange={(e) => setValues((v) => ({ ...v, is_active: e.target.checked }))}
        />
        <span>
          Active
          <span className="text-meta text-muted block">
            Deactivating signs them out everywhere. Their records stay.
          </span>
        </span>
      </label>
      <FormError error={error} />
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button type="button" variant="quiet" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" busy={save.isPending}>
          Save changes
        </Button>
      </div>
    </form>
  );
}

const KIND_LABELS: Record<StageKind, string> = {
  open: "Open",
  won: "Closes as won",
  lost: "Closes as lost",
};

function PipelineSection() {
  const stages = useStages();
  const client = useQueryClient();
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<Stage | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  const reorder = useMutation({
    mutationFn: (ids: string[]) => put<Stage[]>("/stages/order", { stage_ids: ids }),
    onSuccess: (saved) => client.setQueryData(keys.stages, saved),
    onSettled: () => client.invalidateQueries({ queryKey: ["board"] }),
  });
  const remove = useMutation({
    mutationFn: (id: string) => del(`/stages/${id}`),
    onMutate: () => setProblem(null),
    onSuccess: () => client.invalidateQueries({ queryKey: keys.stages }),
    onError: (err) =>
      setProblem(err instanceof ApiError ? err.message : "Could not delete the stage."),
  });

  function shift(list: Stage[], index: number, by: number) {
    const ids = list.map((s) => s.id);
    const [moved] = ids.splice(index, 1);
    if (moved) ids.splice(index + by, 0, moved);
    reorder.mutate(ids);
  }

  return (
    <Panel>
      <div className="flex items-center justify-between gap-3 p-4">
        <div>
          <h2 className="text-title font-semibold">Pipeline stages</h2>
          <p className="text-body text-muted">Probability drives the weighted forecast.</p>
        </div>
        <Button onClick={() => setAdding(true)}>Add stage</Button>
      </div>
      {problem ? (
        <p
          role="alert"
          className="rounded-control bg-critical-soft text-body text-critical mx-4 mb-3 px-3 py-2"
        >
          {problem}
        </p>
      ) : null}
      {stages.isPending ? (
        <Loading label="Loading stages…" />
      ) : stages.isError ? (
        <ErrorState error={stages.error} onRetry={() => stages.refetch()} />
      ) : (
        <ol className="divide-line border-line divide-y border-t">
          {stages.data.map((stage, i, list) => (
            <li key={stage.id} className="flex flex-wrap items-center gap-3 px-4 py-3">
              <div className="min-w-0 flex-1">
                <p className="font-medium">{stage.name}</p>
                <p className="num text-meta text-muted">
                  {KIND_LABELS[stage.kind]}, {stage.probability}%
                </p>
              </div>
              <div className="flex gap-1">
                <Button
                  variant="quiet"
                  aria-label={`Move ${stage.name} up`}
                  disabled={i === 0 || reorder.isPending}
                  onClick={() => shift(list, i, -1)}
                >
                  Up
                </Button>
                <Button
                  variant="quiet"
                  aria-label={`Move ${stage.name} down`}
                  disabled={i === list.length - 1 || reorder.isPending}
                  onClick={() => shift(list, i, 1)}
                >
                  Down
                </Button>
                <Button variant="quiet" onClick={() => setEditing(stage)}>
                  Edit
                </Button>
                <Button
                  variant="quiet"
                  className="hover:text-critical"
                  onClick={() => {
                    if (window.confirm(`Delete the ${stage.name} stage?`)) remove.mutate(stage.id);
                  }}
                >
                  Delete
                </Button>
              </div>
            </li>
          ))}
        </ol>
      )}
      <Dialog open={adding} onClose={() => setAdding(false)} title="Add a stage">
        {adding ? <StageForm onClose={() => setAdding(false)} /> : null}
      </Dialog>
      <Dialog open={editing !== null} onClose={() => setEditing(null)} title="Edit stage">
        {editing ? <StageForm stage={editing} onClose={() => setEditing(null)} /> : null}
      </Dialog>
    </Panel>
  );
}

function StageForm({ stage, onClose }: { stage?: Stage; onClose: () => void }) {
  const client = useQueryClient();
  const [values, setValues] = useState({
    name: stage?.name ?? "",
    probability: String(stage?.probability ?? 50),
    kind: stage?.kind ?? ("open" as StageKind),
  });
  const save = useMutation({
    mutationFn: () => {
      const body = { name: values.name, probability: Number(values.probability) };
      return stage
        ? put<Stage>(`/stages/${stage.id}`, body)
        : post<Stage>("/stages", { ...body, kind: values.kind });
    },
    onSuccess: () => {
      client.invalidateQueries({ queryKey: keys.stages });
      client.invalidateQueries({ queryKey: ["board"] });
      onClose();
    },
  });
  const error = save.error;
  return (
    <form
      noValidate
      className="flex flex-col gap-4"
      onSubmit={(e: FormEvent) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <Field label="Stage name" error={fieldError(error, "name")}>
        {(p) => (
          <TextInput
            {...p}
            value={values.name}
            onChange={(e) => setValues((v) => ({ ...v, name: e.target.value }))}
            autoFocus
          />
        )}
      </Field>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Win probability (%)" error={fieldError(error, "probability")}>
          {(p) => (
            <TextInput
              {...p}
              type="number"
              min={0}
              max={100}
              className="num"
              value={values.probability}
              onChange={(e) => setValues((v) => ({ ...v, probability: e.target.value }))}
            />
          )}
        </Field>
        {!stage ? (
          <Field label="Type">
            {(p) => (
              <Select
                {...p}
                value={values.kind}
                onChange={(e) => setValues((v) => ({ ...v, kind: e.target.value as StageKind }))}
              >
                <option value="open">Open</option>
                <option value="won">Closes as won</option>
                <option value="lost">Closes as lost</option>
              </Select>
            )}
          </Field>
        ) : null}
      </div>
      <FormError error={error} />
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button type="button" variant="quiet" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" busy={save.isPending}>
          {stage ? "Save stage" : "Add stage"}
        </Button>
      </div>
    </form>
  );
}
