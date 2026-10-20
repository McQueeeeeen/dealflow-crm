"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { OwnerOptions } from "@/components/owner-select";
import { CompanySelect, ContactSelect } from "@/components/record-pickers";
import { Dialog, Field, ModalForm, Select, TextInput, fieldError } from "@/components/ui";
import { ApiError, post, put } from "@/lib/api";
import { centsToInput, parseMoneyInput } from "@/lib/format";
import { useMe, useStages } from "@/lib/queries";
import type { Deal } from "@/lib/types";

interface Props {
  open: boolean;
  onClose: () => void;
  deal?: Deal;
  defaults?: { stageId?: string; companyId?: string; contactId?: string };
  onSaved?: (deal: Deal) => void;
}

export function DealFormDialog(props: Props) {
  return (
    <Dialog open={props.open} onClose={props.onClose} title={props.deal ? "Edit Deal" : "New Deal"}>
      {props.open ? <DealForm {...props} /> : null}
    </Dialog>
  );
}

function DealForm({ deal, defaults, onClose, onSaved }: Props) {
  const me = useMe().data;
  const stages = useStages().data ?? [];
  const client = useQueryClient();
  const [values, setValues] = useState({
    title: deal?.title ?? "",
    value: deal ? centsToInput(deal.value_cents) : "",
    stage_id: defaults?.stageId ?? "",
    company_id: deal?.company_id ?? defaults?.companyId ?? "",
    contact_id: deal?.contact_id ?? defaults?.contactId ?? "",
    owner_id: deal?.owner_id ?? me?.id ?? "",
    expected_close_date: deal?.expected_close_date ?? "",
  });
  const [valueError, setValueError] = useState<string>();

  const save = useMutation({
    mutationFn: (body: Record<string, unknown>) =>
      deal ? put<Deal>(`/deals/${deal.id}`, body) : post<Deal>("/deals", body),
    onSuccess: (saved) => {
      client.invalidateQueries({ queryKey: ["board"] });
      client.invalidateQueries({ queryKey: ["deals"] });
      client.invalidateQueries({ queryKey: ["analytics"] });
      client.invalidateQueries({ queryKey: ["audit"] });
      client.setQueryData(["deal", saved.id], saved);
      onSaved?.(saved);
      onClose();
    },
    onError: (err) => {
      if (err instanceof ApiError && err.status === 409 && deal) {
        client.invalidateQueries({ queryKey: ["deal", deal.id] });
        client.invalidateQueries({ queryKey: ["board"] });
      }
    },
  });

  function submit() {
    const cents = parseMoneyInput(values.value);
    if (cents === null) {
      setValueError("Enter an amount like 12500 or 12,500.50");
      return;
    }
    setValueError(undefined);
    save.mutate({
      title: values.title,
      value_cents: cents,
      company_id: values.company_id || null,
      contact_id: values.contact_id || null,
      owner_id: values.owner_id || null,
      expected_close_date: values.expected_close_date || null,
      ...(deal ? { version: deal.version } : { stage_id: values.stage_id || null }),
    });
  }

  const set = (key: keyof typeof values) => (value: string) =>
    setValues((v) => ({ ...v, [key]: value }));
  const error = save.error;

  return (
    <ModalForm
      onSubmit={submit}
      onClose={onClose}
      error={error}
      busy={save.isPending}
      submitLabel={deal ? "Save Deal" : "Create Deal"}
    >
      <Field label="Deal name" name="title" error={fieldError(error, "title")}>
        {(p) => (
          <TextInput
            {...p}
            value={values.title}
            onChange={(e) => set("title")(e.target.value)}
            autoFocus
          />
        )}
      </Field>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field
          label={`Value (${me?.currency ?? "USD"})`}
          name="value"
          error={valueError ?? fieldError(error, "value_cents")}
        >
          {(p) => (
            <TextInput
              {...p}
              inputMode="decimal"
              className="num"
              value={values.value}
              onChange={(e) => set("value")(e.target.value)}
              placeholder="12,500"
            />
          )}
        </Field>
        <Field
          label="Expected close"
          name="expected_close_date"
          error={fieldError(error, "expected_close_date")}
        >
          {(p) => (
            <TextInput
              {...p}
              type="date"
              value={values.expected_close_date}
              onChange={(e) => set("expected_close_date")(e.target.value)}
            />
          )}
        </Field>
      </div>
      {!deal ? (
        <Field label="Stage" name="stage_id" error={fieldError(error, "stage_id")}>
          {(p) => (
            <Select
              {...p}
              value={values.stage_id}
              onChange={(e) => set("stage_id")(e.target.value)}
            >
              <option value="">First stage of the pipeline</option>
              {stages.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </Select>
          )}
        </Field>
      ) : null}
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Company" name="company_id" error={fieldError(error, "company_id")}>
          {(p) => <CompanySelect {...p} value={values.company_id} onChange={set("company_id")} />}
        </Field>
        <Field label="Contact" name="contact_id" error={fieldError(error, "contact_id")}>
          {(p) => (
            <ContactSelect
              {...p}
              value={values.contact_id}
              onChange={set("contact_id")}
              companyId={values.company_id || undefined}
            />
          )}
        </Field>
      </div>
      <Field label="Owner" name="owner_id" error={fieldError(error, "owner_id")}>
        {(p) => (
          <Select {...p} value={values.owner_id} onChange={(e) => set("owner_id")(e.target.value)}>
            <OwnerOptions />
          </Select>
        )}
      </Field>
    </ModalForm>
  );
}
