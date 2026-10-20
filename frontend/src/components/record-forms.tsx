"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";

import { OwnerOptions } from "@/components/owner-select";
import { CompanySelect } from "@/components/record-pickers";
import { Button, Dialog, Field, FormError, Select, TextInput, fieldError } from "@/components/ui";
import { post, put } from "@/lib/api";
import { useMe } from "@/lib/queries";
import type { Company, Contact } from "@/lib/types";

function Actions({ onClose, busy, label }: { onClose: () => void; busy: boolean; label: string }) {
  return (
    <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
      <Button type="button" variant="quiet" onClick={onClose}>
        Cancel
      </Button>
      <Button type="submit" variant="primary" busy={busy}>
        {label}
      </Button>
    </div>
  );
}

interface CompanyProps {
  open: boolean;
  onClose: () => void;
  company?: Company;
  onSaved?: (company: Company) => void;
}

export function CompanyFormDialog(props: CompanyProps) {
  return (
    <Dialog open={props.open} onClose={props.onClose} title={props.company ? "Edit company" : "New company"}>
      {props.open ? <CompanyForm {...props} /> : null}
    </Dialog>
  );
}

function CompanyForm({ company, onClose, onSaved }: CompanyProps) {
  const me = useMe().data;
  const client = useQueryClient();
  const [values, setValues] = useState({
    name: company?.name ?? "",
    domain: company?.domain ?? "",
    industry: company?.industry ?? "",
    owner_id: company?.owner_id ?? me?.id ?? "",
  });
  const save = useMutation({
    mutationFn: () =>
      company ? put<Company>(`/companies/${company.id}`, values) : post<Company>("/companies", values),
    onSuccess: (saved) => {
      client.invalidateQueries({ queryKey: ["companies"] });
      client.invalidateQueries({ queryKey: ["audit"] });
      client.setQueryData(["company", saved.id], saved);
      onSaved?.(saved);
      onClose();
    },
  });
  const error = save.error;
  const set = (key: keyof typeof values) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  return (
    <form
      onSubmit={(e: FormEvent) => {
        e.preventDefault();
        save.mutate();
      }}
      noValidate
      className="flex flex-col gap-4"
    >
      <Field label="Company name" error={fieldError(error, "name")}>
        {(p) => <TextInput {...p} value={values.name} onChange={set("name")} autoFocus />}
      </Field>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Website domain" error={fieldError(error, "domain")}>
          {(p) => <TextInput {...p} value={values.domain} onChange={set("domain")} placeholder="example.com" />}
        </Field>
        <Field label="Industry" error={fieldError(error, "industry")}>
          {(p) => <TextInput {...p} value={values.industry} onChange={set("industry")} />}
        </Field>
      </div>
      <Field label="Owner" error={fieldError(error, "owner_id")}>
        {(p) => (
          <Select {...p} value={values.owner_id} onChange={set("owner_id")}>
            <OwnerOptions />
          </Select>
        )}
      </Field>
      <FormError error={error} />
      <Actions onClose={onClose} busy={save.isPending} label={company ? "Save company" : "Add company"} />
    </form>
  );
}

interface ContactProps {
  open: boolean;
  onClose: () => void;
  contact?: Contact;
  companyId?: string;
  onSaved?: (contact: Contact) => void;
}

export function ContactFormDialog(props: ContactProps) {
  return (
    <Dialog open={props.open} onClose={props.onClose} title={props.contact ? "Edit contact" : "New contact"}>
      {props.open ? <ContactForm {...props} /> : null}
    </Dialog>
  );
}

function ContactForm({ contact, companyId, onClose, onSaved }: ContactProps) {
  const me = useMe().data;
  const client = useQueryClient();
  const [values, setValues] = useState({
    first_name: contact?.first_name ?? "",
    last_name: contact?.last_name ?? "",
    email: contact?.email ?? "",
    phone: contact?.phone ?? "",
    job_title: contact?.job_title ?? "",
    company_id: contact?.company_id ?? companyId ?? "",
    owner_id: contact?.owner_id ?? me?.id ?? "",
  });
  const save = useMutation({
    mutationFn: () => {
      const body = { ...values, company_id: values.company_id || null };
      return contact ? put<Contact>(`/contacts/${contact.id}`, body) : post<Contact>("/contacts", body);
    },
    onSuccess: (saved) => {
      client.invalidateQueries({ queryKey: ["contacts"] });
      client.invalidateQueries({ queryKey: ["companies"] });
      client.invalidateQueries({ queryKey: ["audit"] });
      client.setQueryData(["contact", saved.id], saved);
      onSaved?.(saved);
      onClose();
    },
  });
  const error = save.error;
  const set = (key: keyof typeof values) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  return (
    <form
      onSubmit={(e: FormEvent) => {
        e.preventDefault();
        save.mutate();
      }}
      noValidate
      className="flex flex-col gap-4"
    >
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="First name" error={fieldError(error, "first_name")}>
          {(p) => <TextInput {...p} value={values.first_name} onChange={set("first_name")} autoFocus />}
        </Field>
        <Field label="Last name" error={fieldError(error, "last_name")}>
          {(p) => <TextInput {...p} value={values.last_name} onChange={set("last_name")} />}
        </Field>
        <Field label="Email" error={fieldError(error, "email")}>
          {(p) => (
            <TextInput {...p} type="email" value={values.email} onChange={set("email")} placeholder="name@company.com" />
          )}
        </Field>
        <Field label="Phone" error={fieldError(error, "phone")}>
          {(p) => <TextInput {...p} type="tel" value={values.phone} onChange={set("phone")} />}
        </Field>
        <Field label="Job title" error={fieldError(error, "job_title")}>
          {(p) => <TextInput {...p} value={values.job_title} onChange={set("job_title")} />}
        </Field>
        <Field label="Company" error={fieldError(error, "company_id")}>
          {(p) => (
            <CompanySelect
              {...p}
              value={values.company_id}
              onChange={(value) => setValues((v) => ({ ...v, company_id: value }))}
            />
          )}
        </Field>
      </div>
      <Field label="Owner" error={fieldError(error, "owner_id")}>
        {(p) => (
          <Select {...p} value={values.owner_id} onChange={set("owner_id")}>
            <OwnerOptions />
          </Select>
        )}
      </Field>
      <FormError error={error} />
      <Actions onClose={onClose} busy={save.isPending} label={contact ? "Save contact" : "Add contact"} />
    </form>
  );
}
