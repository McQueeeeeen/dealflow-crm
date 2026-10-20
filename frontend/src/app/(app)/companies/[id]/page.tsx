"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useState } from "react";

import { ActivityPanel } from "@/components/activity";
import { DealFormDialog } from "@/components/deal-form";
import { LinkedDeals } from "@/components/deal-list";
import { NotesPanel } from "@/components/notes";
import { CompanyFormDialog, ContactFormDialog } from "@/components/record-forms";
import { Button, ErrorState, Loading, Panel, RecordView } from "@/components/ui";
import { del, get, query } from "@/lib/api";
import { fullName } from "@/lib/format";
import { useMoney } from "@/lib/hooks";
import { canModify, isManager, keys, useMe } from "@/lib/queries";
import type { Company, Contact, Page } from "@/lib/types";

export default function CompanyPage() {
  const { id } = useParams<{ id: string }>();
  const company = useQuery({
    queryKey: keys.company(id),
    queryFn: () => get<Company>(`/companies/${id}`),
  });
  return (
    <RecordView
      query={company}
      missing={{
        title: "This company no longer exists",
        href: "/companies",
        back: "Back to companies",
      }}
    >
      {(company) => <CompanyView company={company} />}
    </RecordView>
  );
}

function CompanyView({ company }: { company: Company }) {
  const me = useMe().data;
  const fmt = useMoney();
  const router = useRouter();
  const client = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [addingContact, setAddingContact] = useState(false);
  const [addingDeal, setAddingDeal] = useState(false);
  const editable = canModify(me, company.owner_id);

  const remove = useMutation({
    mutationFn: () => del(`/companies/${company.id}`),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: ["companies"] });
      router.push("/companies");
    },
  });

  return (
    <>
      <Link
        href="/companies"
        className="text-body text-muted hover:text-ink underline underline-offset-4"
      >
        Companies
      </Link>
      <header className="mt-3 flex flex-wrap items-start justify-between gap-x-8 gap-y-4 pb-6">
        <div className="min-w-0">
          <h1 className="text-display font-semibold tracking-tight break-words">{company.name}</h1>
          <p className="text-muted mt-1">
            {[company.industry, company.domain].filter(Boolean).join(", ") ||
              "No industry or domain yet"}
          </p>
          <p className="num text-lead mt-3 font-display">
            {company.open_deals_count > 0
              ? `${fmt(company.open_deals_value_cents)} open across ${company.open_deals_count} ${company.open_deals_count === 1 ? "deal" : "deals"}`
              : "No open deals"}
            <span className="text-muted">, owned by {company.owner_name}</span>
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="primary" onClick={() => setAddingDeal(true)}>
            New deal
          </Button>
          {editable ? <Button onClick={() => setEditing(true)}>Edit company</Button> : null}
          {isManager(me) ? (
            <Button
              variant="danger"
              busy={remove.isPending}
              onClick={() => {
                if (
                  window.confirm(`Delete ${company.name}? Its deals and contacts stay, unlinked.`)
                )
                  remove.mutate();
              }}
            >
              Delete
            </Button>
          ) : null}
        </div>
      </header>

      <div className="grid items-start gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
        <div className="flex flex-col gap-6">
          <Panel>
            <h2 className="text-title px-4 pt-4 font-semibold">Deals</h2>
            <div className="border-line mt-2 border-t">
              <LinkedDeals filter={{ company_id: company.id }} />
            </div>
          </Panel>
          <NotesPanel parent={{ company_id: company.id }} />
        </div>
        <div className="flex flex-col gap-6">
          <Panel quiet>
            <div className="flex items-center justify-between gap-3 px-4 pt-4">
              <h2 className="text-title font-semibold">People</h2>
              <Button variant="quiet" onClick={() => setAddingContact(true)}>
                Add contact
              </Button>
            </div>
            <CompanyContacts companyId={company.id} />
          </Panel>
          <ActivityPanel entityType="company" entityId={company.id} />
        </div>
      </div>

      <CompanyFormDialog open={editing} onClose={() => setEditing(false)} company={company} />
      <ContactFormDialog
        open={addingContact}
        onClose={() => setAddingContact(false)}
        companyId={company.id}
      />
      <DealFormDialog
        open={addingDeal}
        onClose={() => setAddingDeal(false)}
        defaults={{ companyId: company.id }}
        onSaved={() => client.invalidateQueries({ queryKey: keys.company(company.id) })}
      />
    </>
  );
}

function CompanyContacts({ companyId }: { companyId: string }) {
  const contacts = useQuery({
    queryKey: keys.contacts({ company: companyId }),
    queryFn: () =>
      get<Page<Contact>>(`/contacts${query({ company_id: companyId, per_page: 100 })}`),
  });
  if (contacts.isPending) return <Loading label="Loading people…" />;
  if (contacts.isError)
    return <ErrorState error={contacts.error} onRetry={() => contacts.refetch()} />;
  if (contacts.data.data.length === 0) {
    return <p className="text-body text-muted px-4 py-6">Nobody from this company yet.</p>;
  }
  return (
    <ul className="divide-line border-line mt-2 divide-y border-t">
      {contacts.data.data.map((c) => (
        <li key={c.id} className="px-4 py-3">
          <Link
            href={`/contacts/${c.id}`}
            className="font-medium hover:underline hover:underline-offset-2"
          >
            {fullName(c)}
          </Link>
          {c.job_title ? <p className="text-meta text-muted">{c.job_title}</p> : null}
          {c.email ? (
            <a
              href={`mailto:${c.email}`}
              className="text-meta text-accent underline underline-offset-2"
            >
              {c.email}
            </a>
          ) : null}
        </li>
      ))}
    </ul>
  );
}
