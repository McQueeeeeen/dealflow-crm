"use client";

import { useQuery } from "@tanstack/react-query";

import { Select } from "@/components/ui";
import { get, query } from "@/lib/api";
import { fullName } from "@/lib/format";
import { keys } from "@/lib/queries";
import type { Company, Contact, Page } from "@/lib/types";

type FieldProps = { id: string; "aria-invalid"?: boolean; "aria-describedby"?: string };

// Small workspaces fit in one page; larger ones would need a typeahead.
export function CompanySelect({
  value,
  onChange,
  ...field
}: FieldProps & { value: string; onChange: (value: string) => void }) {
  const companies = useQuery({
    queryKey: keys.companies({ picker: true }),
    queryFn: () => get<Page<Company>>(`/companies${query({ per_page: 100 })}`),
  });
  return (
    <Select {...field} value={value} onChange={(e) => onChange(e.target.value)}>
      <option value="">No company</option>
      {companies.data?.data.map((c) => (
        <option key={c.id} value={c.id}>
          {c.name}
        </option>
      ))}
    </Select>
  );
}

export function ContactSelect({
  value,
  onChange,
  companyId,
  ...field
}: FieldProps & { value: string; onChange: (value: string) => void; companyId?: string }) {
  const contacts = useQuery({
    queryKey: keys.contacts({ picker: true, companyId }),
    queryFn: () =>
      get<Page<Contact>>(`/contacts${query({ per_page: 100, company_id: companyId })}`),
  });
  return (
    <Select {...field} value={value} onChange={(e) => onChange(e.target.value)}>
      <option value="">No contact</option>
      {contacts.data?.data.map((c) => (
        <option key={c.id} value={c.id}>
          {fullName(c)}
          {c.company_name ? `, ${c.company_name}` : ""}
        </option>
      ))}
    </Select>
  );
}
