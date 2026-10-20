"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { Table } from "@/components/table";
import {
  Avatar,
  Button,
  Dialog,
  ErrorState,
  Field,
  Loading,
  ModalForm,
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
        <div className="grid gap-6">
          <PipelineSection />
          <WorkspaceSection />
        </div>
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
          Add Teammate
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
                  <p className="flex items-center gap-2 font-medium">
                    <Avatar name={u.full_name} />
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
      <Dialog open={adding} onClose={() => setAdding(false)} title="Add a Teammate">
        {adding ? <AddUserForm onClose={() => setAdding(false)} /> : null}
      </Dialog>
      <Dialog open={editing !== null} onClose={() => setEditing(null)} title="Edit Teammate">
        {editing ? <EditUserForm user={editing} onClose={() => setEditing(null)} /> : null}
      </Dialog>
    </Panel>
  );
}

function RoleSelect(props: {
  id: string;
  name?: string;
  value: Role;
  onChange: (role: Role) => void;
}) {
  return (
    <Select
      id={props.id}
      name={props.name}
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
    <ModalForm
      onSubmit={() => save.mutate()}
      onClose={onClose}
      error={error}
      busy={save.isPending}
      submitLabel="Add Teammate"
    >
      <Field label="Full name" name="full_name" error={fieldError(error, "full_name")}>
        {(p) => (
          <TextInput
            {...p}
            value={values.full_name}
            onChange={(e) => setValues((v) => ({ ...v, full_name: e.target.value }))}
            autoFocus
          />
        )}
      </Field>
      <Field label="Work email" name="email" error={fieldError(error, "email")}>
        {(p) => (
          <TextInput
            {...p}
            type="email"
            value={values.email}
            onChange={(e) => setValues((v) => ({ ...v, email: e.target.value }))}
          />
        )}
      </Field>
      <Field label="Role" name="role">
        {(p) => (
          <RoleSelect
            id={p.id}
            name={p.name}
            value={values.role}
            onChange={(role) => setValues((v) => ({ ...v, role }))}
          />
        )}
      </Field>
      <Field
        label="Initial password"
        name="password"
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
    </ModalForm>
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
    <ModalForm
      onSubmit={() => save.mutate()}
      onClose={onClose}
      error={error}
      busy={save.isPending}
      submitLabel="Save Changes"
    >
      <Field label="Full name" name="full_name" error={fieldError(error, "full_name")}>
        {(p) => (
          <TextInput
            {...p}
            value={values.full_name}
            onChange={(e) => setValues((v) => ({ ...v, full_name: e.target.value }))}
          />
        )}
      </Field>
      <Field label="Role" name="role">
        {(p) => (
          <RoleSelect
            id={p.id}
            name={p.name}
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
    </ModalForm>
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
          <h2 className="text-title font-semibold">Pipeline Stages</h2>
          <p className="text-body text-muted">Probability drives the weighted forecast.</p>
        </div>
        <Button onClick={() => setAdding(true)}>Add Stage</Button>
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
        <ol className="row-list">
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
      <Dialog open={adding} onClose={() => setAdding(false)} title="Add a Stage">
        {adding ? <StageForm onClose={() => setAdding(false)} /> : null}
      </Dialog>
      <Dialog open={editing !== null} onClose={() => setEditing(null)} title="Edit Stage">
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
    <ModalForm
      onSubmit={() => save.mutate()}
      onClose={onClose}
      error={error}
      busy={save.isPending}
      submitLabel={stage ? "Save Stage" : "Add Stage"}
    >
      <Field label="Stage name" name="name" error={fieldError(error, "name")}>
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
        <Field
          label="Win probability (%)"
          name="probability"
          error={fieldError(error, "probability")}
        >
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
          <Field label="Type" name="kind">
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
    </ModalForm>
  );
}

function WorkspaceSection() {
  const me = useMe().data;
  const client = useQueryClient();
  const [days, setDays] = useState(String(me?.stale_after_days ?? 14));
  const save = useMutation({
    mutationFn: () => put("/organization", { stale_after_days: Number(days) }),
    onSuccess: () => client.invalidateQueries({ queryKey: keys.me }),
  });
  return (
    <Panel className="p-4">
      <h2 className="text-title font-semibold">Going Cold</h2>
      <p className="text-body text-muted mt-1">
        Open deals with no notes, completed follow-ups or changes for this long are flagged on the
        board and on Today.
      </p>
      <form
        className="mt-4 flex items-end gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <Field
          label="Days without activity"
          name="stale_after_days"
          error={fieldError(save.error, "stale_after_days")}
        >
          {(p) => (
            <TextInput
              {...p}
              type="number"
              min={1}
              max={365}
              inputMode="numeric"
              value={days}
              onChange={(e) => {
                save.reset();
                setDays(e.target.value);
              }}
              className="num w-28"
            />
          )}
        </Field>
        <Button type="submit" busy={save.isPending}>
          Save
        </Button>
      </form>
      {save.isSuccess ? (
        <p role="status" className="text-meta text-won mt-2">
          Saved. Deals quiet for {days} days now count as going cold.
        </p>
      ) : null}
    </Panel>
  );
}
