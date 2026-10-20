"use client";

import { keepPreviousData, useQuery } from "@tanstack/react-query";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";

import { OwnerFilter } from "@/components/owner-select";
import { CompanyFormDialog } from "@/components/record-forms";
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
import { useDebounced, useMoney } from "@/lib/hooks";
import { keys } from "@/lib/queries";
import type { Company, Page } from "@/lib/types";

export default function CompaniesPage() {
  const router = useRouter();
  const fmt = useMoney();
  const [search, setSearch] = useState("");
  const [owner, setOwner] = useState("");
  const [page, setPage] = useState(1);
  const [creating, setCreating] = useState(false);
  const q = useDebounced(search.trim());

  const companies = useQuery({
    queryKey: keys.companies({ q, owner, page }),
    queryFn: () => get<Page<Company>>(`/companies${query({ q, owner_id: owner, page })}`),
    placeholderData: keepPreviousData,
  });

  return (
    <>
      <PageHeader
        title="Companies"
        actions={
          <Button variant="primary" onClick={() => setCreating(true)}>
            Add company
          </Button>
        }
      />
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <label className="min-w-0 flex-1 sm:max-w-sm">
          <span className="sr-only">Search companies</span>
          <TextInput
            type="search"
            placeholder="Search by name or domain"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setPage(1);
            }}
          />
        </label>
        <OwnerFilter
          value={owner}
          onChange={(v) => {
            setOwner(v);
            setPage(1);
          }}
        />
      </div>
      <Panel>
        {companies.isPending ? (
          <Loading label="Loading companies…" />
        ) : companies.isError ? (
          <ErrorState error={companies.error} onRetry={() => companies.refetch()} />
        ) : companies.data.total === 0 ? (
          q || owner ? (
            <EmptyState title="No companies match" body="Try a shorter search or another owner." />
          ) : (
            <EmptyState
              title="No companies yet"
              body="Add the organizations you sell to. Contacts and deals can then be linked to them."
              action={<Button onClick={() => setCreating(true)}>Add company</Button>}
            />
          )
        ) : (
          <>
            <Table
              caption="Companies"
              rows={companies.data.data}
              columns={[
                {
                  header: "Name",
                  cell: (c) => (
                    <div className="min-w-0">
                      <Link href={`/companies/${c.id}`} className="font-medium hover:underline hover:underline-offset-2">
                        {c.name}
                      </Link>
                      {c.domain ? <p className="text-meta text-muted">{c.domain}</p> : null}
                    </div>
                  ),
                },
                { header: "Industry", cell: (c) => c.industry ?? "", hideBelow: "lg" },
                { header: "Owner", cell: (c) => c.owner_name, hideBelow: "md" },
                { header: "Contacts", cell: (c) => c.contacts_count, align: "right", hideBelow: "sm" },
                {
                  header: "Open pipeline",
                  align: "right",
                  cell: (c) =>
                    c.open_deals_count > 0 ? (
                      <>
                        {fmt(c.open_deals_value_cents)}
                        <span className="block text-meta text-muted">
                          {c.open_deals_count} {c.open_deals_count === 1 ? "deal" : "deals"}
                        </span>
                      </>
                    ) : (
                      <span className="text-muted">None</span>
                    ),
                },
              ]}
            />
            <Pagination page={page} perPage={companies.data.per_page} total={companies.data.total} onPage={setPage} />
          </>
        )}
      </Panel>
      <CompanyFormDialog
        open={creating}
        onClose={() => setCreating(false)}
        onSaved={(c) => router.push(`/companies/${c.id}`)}
      />
    </>
  );
}
