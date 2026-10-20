"use client";

import { keepPreviousData, useQuery } from "@tanstack/react-query";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";

import { OwnerFilter } from "@/components/owner-select";
import { ContactFormDialog } from "@/components/record-forms";
import { Table } from "@/components/table";
import {
  Button,
  EmptyState,
  ErrorState,
  Loading,
  PageHeader,
  Pagination,
  Panel,
  TextInput,
} from "@/components/ui";
import { get, query } from "@/lib/api";
import { fullName } from "@/lib/format";
import { useUrlParams, useUrlSearch } from "@/lib/hooks";
import { keys } from "@/lib/queries";
import type { Contact, Page } from "@/lib/types";
import { FORWARD } from "@/lib/transitions";

export default function ContactsPage() {
  const router = useRouter();
  const [params, update] = useUrlParams();
  const q = params.get("q") ?? "";
  const owner = params.get("owner") ?? "";
  const page = Number(params.get("page") ?? 1);
  const [search, setSearch] = useUrlSearch(q, update);
  const [creating, setCreating] = useState(false);

  const contacts = useQuery({
    queryKey: keys.contacts({ q, owner, page }),
    queryFn: () => get<Page<Contact>>(`/contacts${query({ q, owner_id: owner, page })}`),
    placeholderData: keepPreviousData,
  });

  return (
    <>
      <PageHeader
        title="Contacts"
        actions={
          <Button variant="primary" onClick={() => setCreating(true)}>
            Add Contact
          </Button>
        }
      />
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <label className="min-w-0 flex-1 sm:max-w-sm">
          <span className="sr-only">Search contacts</span>
          <TextInput
            type="search"
            name="q"
            placeholder="Search by name or email…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </label>
        <OwnerFilter value={owner} onChange={(v) => update({ owner: v, page: null })} />
      </div>
      <Panel>
        {contacts.isPending ? (
          <Loading label="Loading contacts…" />
        ) : contacts.isError ? (
          <ErrorState error={contacts.error} onRetry={() => contacts.refetch()} />
        ) : contacts.data.total === 0 ? (
          q || owner ? (
            <EmptyState title="No contacts match" body="Try a shorter search or another owner." />
          ) : (
            <EmptyState
              title="No contacts yet"
              body="Add the people you talk to. Link them to a company to see everyone at one account together."
              action={<Button onClick={() => setCreating(true)}>Add Contact</Button>}
            />
          )
        ) : (
          <>
            <Table
              caption="Contacts"
              rows={contacts.data.data}
              columns={[
                {
                  header: "Name",
                  cell: (c) => (
                    <div className="min-w-0">
                      <Link
                        transitionTypes={FORWARD}
                        href={`/contacts/${c.id}`}
                        className="font-medium hover:underline hover:underline-offset-2"
                      >
                        {fullName(c)}
                      </Link>
                      {c.job_title ? <p className="text-meta text-muted">{c.job_title}</p> : null}
                    </div>
                  ),
                },
                {
                  header: "Company",
                  cell: (c) =>
                    c.company_id ? (
                      <Link
                        transitionTypes={FORWARD}
                        href={`/companies/${c.company_id}`}
                        className="hover:underline hover:underline-offset-2"
                      >
                        {c.company_name}
                      </Link>
                    ) : (
                      <span className="text-muted">None</span>
                    ),
                  hideBelow: "sm",
                },
                {
                  header: "Email",
                  cell: (c) =>
                    c.email ? (
                      <a
                        href={`mailto:${c.email}`}
                        className="text-link break-all underline underline-offset-2"
                      >
                        {c.email}
                      </a>
                    ) : null,
                  hideBelow: "md",
                },
                {
                  header: "Phone",
                  cell: (c) => <span className="num whitespace-nowrap">{c.phone}</span>,
                  hideBelow: "lg",
                },
                { header: "Owner", cell: (c) => c.owner_name, hideBelow: "lg" },
              ]}
            />
            <Pagination
              page={page}
              perPage={contacts.data.per_page}
              total={contacts.data.total}
              onPage={(p) => update({ page: p === 1 ? null : p })}
            />
          </>
        )}
      </Panel>
      <ContactFormDialog
        open={creating}
        onClose={() => setCreating(false)}
        onSaved={(c) => router.push(`/contacts/${c.id}`)}
      />
    </>
  );
}
