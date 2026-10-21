"use client";

import { IconTrash } from "@tabler/icons-react";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import { Marked } from "@/components/filter-bar";
import {
  Button,
  Dialog,
  Field,
  ModalForm,
  Select,
  TextArea,
  TextInput,
  fieldError,
} from "@/components/ui";
import { ApiError, post, put } from "@/lib/api";
import { useInvalidate, useStages, useUsers } from "@/lib/queries";
import type { Action, Rule, Stage, Trigger, User } from "@/lib/types";

const ACTIONS: Record<Action["type"], string> = {
  create_task: "Create a follow-up",
  add_note: "Add a note",
  assign_owner: "Reassign the deal",
  set_close_date: "Set the expected close",
};

function blank(type: Action["type"], users: User[]): Action {
  switch (type) {
    case "create_task":
      return { type, title: "Follow up on {deal}", due_in_days: 2, assignee_id: null };
    case "add_note":
      return { type, body: "" };
    case "assign_owner":
      return { type, user_id: users[0]?.id ?? "" };
    case "set_close_date":
      return { type, days_from_now: 30 };
  }
}

export function triggerText(trigger: Trigger, stages: Stage[]): string {
  switch (trigger.type) {
    case "deal_created":
      return "a deal is created";
    case "stage_entered":
      return `a deal enters ${stages.find((s) => s.id === trigger.stage_id)?.name ?? "a removed stage"}`;
    case "task_completed":
      return "a follow-up on a deal is completed";
  }
}

export function actionText(action: Action, users: User[]): string {
  const name = (id: string | null) =>
    id ? (users.find((u) => u.id === id)?.full_name ?? "a former teammate") : "the deal owner";
  switch (action.type) {
    case "create_task":
      return `Create the follow-up “${action.title}” for ${name(action.assignee_id)}, due in ${action.due_in_days} ${action.due_in_days === 1 ? "day" : "days"}`;
    case "add_note":
      return `Add the note “${action.body}”`;
    case "assign_owner":
      return `Reassign the deal to ${name(action.user_id)}`;
    case "set_close_date":
      return `Set the expected close ${action.days_from_now} days out`;
  }
}

export function RuleDialog({ rule, onClose }: { rule: Rule | "new" | null; onClose: () => void }) {
  return (
    <Dialog
      open={rule !== null}
      onClose={onClose}
      title={rule === "new" ? "New Automation" : "Edit Automation"}
      wide
    >
      {rule !== null ? (
        <RuleForm rule={rule === "new" ? undefined : rule} onClose={onClose} />
      ) : null}
    </Dialog>
  );
}

function RuleForm({ rule, onClose }: { rule?: Rule; onClose: () => void }) {
  const stages = useStages().data ?? [];
  const users = (useUsers().data ?? []).filter((u) => u.is_active);
  const invalidate = useInvalidate();
  const [name, setName] = useState(rule?.name ?? "");
  const [trigger, setTrigger] = useState<Trigger>(rule?.trigger ?? { type: "deal_created" });
  const [condition, setCondition] = useState(rule?.condition ?? "");
  const [actions, setActions] = useState<Action[]>(rule?.actions ?? [blank("create_task", [])]);

  const save = useMutation({
    mutationFn: () => {
      const body = { name, enabled: rule?.enabled ?? true, trigger, condition, actions };
      return rule ? put<Rule>(`/automations/${rule.id}`, body) : post<Rule>("/automations", body);
    },
    onSuccess: () => {
      invalidate("rule");
      onClose();
    },
  });
  const error = save.error;
  const filterError =
    error instanceof ApiError && error.code === "invalid_filter" ? error : undefined;
  const replace = (index: number, next: Action) =>
    setActions((list) => list.map((a, i) => (i === index ? next : a)));

  return (
    <ModalForm
      onSubmit={() => save.mutate()}
      onClose={onClose}
      error={filterError ? null : error}
      busy={save.isPending}
      submitLabel={rule ? "Save Automation" : "Create Automation"}
    >
      <Field label="Name" name="name" error={fieldError(error, "name")}>
        {(p) => (
          <TextInput
            {...p}
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Big proposals get a follow-up"
          />
        )}
      </Field>

      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="When" name="trigger" error={fieldError(error, "trigger")}>
          {(p) => (
            <Select
              {...p}
              value={trigger.type}
              onChange={(e) => {
                const type = e.target.value as Trigger["type"];
                setTrigger(
                  type === "stage_entered"
                    ? { type, stage_id: stages.find((s) => s.kind === "open")?.id ?? "" }
                    : { type },
                );
              }}
            >
              <option value="deal_created">A deal is created</option>
              <option value="stage_entered">A deal enters a stage</option>
              <option value="task_completed">A follow-up on a deal is completed</option>
            </Select>
          )}
        </Field>
        {trigger.type === "stage_entered" ? (
          <Field label="Stage" name="stage">
            {(p) => (
              <Select
                {...p}
                value={trigger.stage_id}
                onChange={(e) => setTrigger({ type: "stage_entered", stage_id: e.target.value })}
              >
                {stages.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </Select>
            )}
          </Field>
        ) : null}
      </div>

      <Field
        label="Only for deals matching (optional)"
        name="condition"
        error={filterError?.message}
        hint="Same language as the pipeline filter, for example value>=50k -owner:me."
      >
        {(p) => (
          <TextInput
            {...p}
            spellCheck={false}
            value={condition}
            onChange={(e) => setCondition(e.target.value)}
            placeholder="value>=50k"
            className="font-mono"
          />
        )}
      </Field>
      {filterError ? (
        <p className="text-meta -mt-2">
          <Marked text={condition.trim()} span={filterError.span} />
        </p>
      ) : null}

      <fieldset className="flex flex-col gap-3">
        <legend className="text-meta text-muted mb-1.5 font-medium">Then, in this order</legend>
        {actions.map((action, index) => (
          <div key={index} className="bg-row flex flex-col gap-3 rounded-xl p-3">
            <div className="flex items-end gap-2">
              <div className="min-w-0 flex-1">
                <Field label={`Step ${index + 1}`} name={`action-${index}`}>
                  {(p) => (
                    <Select
                      {...p}
                      value={action.type}
                      onChange={(e) =>
                        replace(index, blank(e.target.value as Action["type"], users))
                      }
                    >
                      {Object.entries(ACTIONS).map(([type, label]) => (
                        <option key={type} value={type}>
                          {label}
                        </option>
                      ))}
                    </Select>
                  )}
                </Field>
              </div>
              {actions.length > 1 ? (
                <Button
                  type="button"
                  variant="quiet"
                  aria-label={`Remove step ${index + 1}`}
                  onClick={() => setActions((list) => list.filter((_, i) => i !== index))}
                >
                  <IconTrash size={16} stroke={1.75} aria-hidden="true" />
                </Button>
              ) : null}
            </div>
            <ActionFields
              action={action}
              users={users}
              index={index}
              onChange={(next) => replace(index, next)}
            />
          </div>
        ))}
        {fieldError(error, "actions") ? (
          <p className="text-meta text-critical">{fieldError(error, "actions")}</p>
        ) : null}
        {actions.length < 5 ? (
          <Button
            type="button"
            className="self-start"
            onClick={() => setActions((list) => [...list, blank("add_note", users)])}
          >
            Add Step
          </Button>
        ) : null}
      </fieldset>
    </ModalForm>
  );
}

function ActionFields({
  action,
  users,
  index,
  onChange,
}: {
  action: Action;
  users: User[];
  index: number;
  onChange: (next: Action) => void;
}) {
  const people = users.map((u) => (
    <option key={u.id} value={u.id}>
      {u.full_name}
    </option>
  ));
  switch (action.type) {
    case "create_task":
      return (
        <div className="grid gap-3 sm:grid-cols-[minmax(0,2fr)_6rem_minmax(0,1fr)]">
          <Field
            label="Title"
            name={`title-${index}`}
            hint="{deal}, {company}, {owner}, {stage} and {value} are filled in."
          >
            {(p) => (
              <TextInput
                {...p}
                value={action.title}
                onChange={(e) => onChange({ ...action, title: e.target.value })}
              />
            )}
          </Field>
          <Field label="Due in days" name={`due-${index}`}>
            {(p) => (
              <TextInput
                {...p}
                type="number"
                min={0}
                max={365}
                inputMode="numeric"
                className="num"
                value={action.due_in_days}
                onChange={(e) => onChange({ ...action, due_in_days: Number(e.target.value) })}
              />
            )}
          </Field>
          <Field label="For" name={`assignee-${index}`}>
            {(p) => (
              <Select
                {...p}
                value={action.assignee_id ?? ""}
                onChange={(e) => onChange({ ...action, assignee_id: e.target.value || null })}
              >
                <option value="">The deal owner</option>
                {people}
              </Select>
            )}
          </Field>
        </div>
      );
    case "add_note":
      return (
        <Field label="Note" name={`note-${index}`}>
          {(p) => (
            <TextArea
              {...p}
              value={action.body}
              onChange={(e) => onChange({ ...action, body: e.target.value })}
              placeholder="{deal} reached {stage}. Check the proposal template."
            />
          )}
        </Field>
      );
    case "assign_owner":
      return (
        <Field label="New owner" name={`owner-${index}`}>
          {(p) => (
            <Select
              {...p}
              value={action.user_id}
              onChange={(e) => onChange({ ...action, user_id: e.target.value })}
            >
              {people}
            </Select>
          )}
        </Field>
      );
    case "set_close_date":
      return (
        <Field label="Days from today" name={`close-${index}`}>
          {(p) => (
            <TextInput
              {...p}
              type="number"
              min={0}
              max={730}
              inputMode="numeric"
              className="num w-32"
              value={action.days_from_now}
              onChange={(e) => onChange({ ...action, days_from_now: Number(e.target.value) })}
            />
          )}
        </Field>
      );
  }
}
