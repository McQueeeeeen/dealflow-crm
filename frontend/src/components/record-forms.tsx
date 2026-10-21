"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { OwnerOptions } from "@/components/owner-select";
import { CompanySelect } from "@/components/record-pickers";
import { Dialog, Field, ModalForm, Select, TextInput, fieldError } from "@/components/ui";
import { post, put } from "@/lib/api";
import { useInvalidate, useMe } from "@/lib/queries";
import type { Company, Contact } from "@/lib/types";

interface CompanyProps {
  open: boolean;
  onClose: () => void;
  company?: Company;
  onSaved?: (company: Company) => void;
}

export function CompanyFormDialog(props: CompanyProps) {
  return (
    <Dialog
      open={props.open}
      onClose={props.onClose}
      title={props.company ? "Edit Company" : "New Company"}
    >
      {props.open ? <CompanyForm {...props} /> : null}
    </Dialog>
  );
}

function CompanyForm({ company, onClose, onSaved }: CompanyProps) {
  const me = useMe().data;
  const client = useQueryClient();
  const invalidate = useInvalidate();
  const [values, setValues] = useState({
    name: company?.name ?? "",
    domain: company?.domain ?? "",
    industry: company?.industry ?? "",
    owner_id: company?.owner_id ?? me?.id ?? "",
  });
  const save = useMutation({
    mutationFn: () =>
      company
        ? put<Company>(`/companies/${company.id}`, values)
        : post<Company>("/companies", values),
    onSuccess: (saved) => {
      invalidate("company");
      client.setQueryData(["company", saved.id], saved);
      onSaved?.(saved);
      onClose();
    },
  });
  const error = save.error;
  const set = (key: keyof typeof values) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  return (
    <ModalForm
      onSubmit={() => save.mutate()}
      onClose={onClose}
      error={error}
      busy={save.isPending}
      submitLabel={company ? "Save Company" : "Add Company"}
    >
      <Field label="Company name" name="name" error={fieldError(error, "name")}>
        {(p) => <TextInput {...p} value={values.name} onChange={set("name")} autoFocus />}
      </Field>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Website domain" name="domain" error={fieldError(error, "domain")}>
          {(p) => (
            <TextInput
              {...p}
              value={values.domain}
              onChange={set("domain")}
              placeholder="example.com"
            />
          )}
        </Field>
        <Field label="Industry" name="industry" error={fieldError(error, "industry")}>
          {(p) => <TextInput {...p} value={values.industry} onChange={set("industry")} />}
        </Field>
      </div>
      <Field label="Owner" name="owner_id" error={fieldError(error, "owner_id")}>
        {(p) => (
          <Select {...p} value={values.owner_id} onChange={set("owner_id")}>
            <OwnerOptions />
          </Select>
        )}
      </Field>
    </ModalForm>
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
    <Dialog
      open={props.open}
      onClose={props.onClose}
      title={props.contact ? "Edit Contact" : "New Contact"}
    >
      {props.open ? <ContactForm {...props} /> : null}
    </Dialog>
  );
}

function ContactForm({ contact, companyId, onClose, onSaved }: ContactProps) {
  const me = useMe().data;
  const client = useQueryClient();
  const invalidate = useInvalidate();
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
      return contact
        ? put<Contact>(`/contacts/${contact.id}`, body)
        : post<Contact>("/contacts", body);
    },
    onSuccess: (saved) => {
      invalidate("contact");
      client.setQueryData(["contact", saved.id], saved);
      onSaved?.(saved);
      onClose();
    },
  });
  const error = save.error;
  const set = (key: keyof typeof values) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  return (
    <ModalForm
      onSubmit={() => save.mutate()}
      onClose={onClose}
      error={error}
      busy={save.isPending}
      submitLabel={contact ? "Save Contact" : "Add Contact"}
    >
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="First name" name="first_name" error={fieldError(error, "first_name")}>
          {(p) => (
            <TextInput {...p} value={values.first_name} onChange={set("first_name")} autoFocus />
          )}
        </Field>
        <Field label="Last name" name="last_name" error={fieldError(error, "last_name")}>
          {(p) => <TextInput {...p} value={values.last_name} onChange={set("last_name")} />}
        </Field>
        <Field label="Email" name="email" error={fieldError(error, "email")}>
          {(p) => (
            <TextInput
              {...p}
              type="email"
              value={values.email}
              onChange={set("email")}
              placeholder="name@company.com"
            />
          )}
        </Field>
        <Field label="Phone" name="phone" error={fieldError(error, "phone")}>
          {(p) => <TextInput {...p} type="tel" value={values.phone} onChange={set("phone")} />}
        </Field>
        <Field label="Job title" name="job_title" error={fieldError(error, "job_title")}>
          {(p) => <TextInput {...p} value={values.job_title} onChange={set("job_title")} />}
        </Field>
        <Field label="Company" name="company_id" error={fieldError(error, "company_id")}>
          {(p) => (
            <CompanySelect
              {...p}
              value={values.company_id}
              onChange={(value) => setValues((v) => ({ ...v, company_id: value }))}
            />
          )}
        </Field>
      </div>
      <Field label="Owner" name="owner_id" error={fieldError(error, "owner_id")}>
        {(p) => (
          <Select {...p} value={values.owner_id} onChange={set("owner_id")}>
            <OwnerOptions />
          </Select>
        )}
      </Field>
    </ModalForm>
  );
}
