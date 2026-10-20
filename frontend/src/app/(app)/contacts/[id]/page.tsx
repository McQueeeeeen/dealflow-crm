"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useState } from "react";

import { ActivityPanel } from "@/components/activity";
import { DealFormDialog } from "@/components/deal-form";
import { LinkedDeals } from "@/components/deal-list";
import { NotesPanel } from "@/components/notes";
import { ContactFormDialog } from "@/components/record-forms";
import { TaskFormDialog } from "@/components/task-form";
import { TaskRow } from "@/components/task-list";
import { Button, ErrorState, Loading, Panel, RecordView } from "@/components/ui";
import { del, get, query } from "@/lib/api";
import { fullName } from "@/lib/format";
import { canModify, isManager, keys, useMe } from "@/lib/queries";
import type { Contact, Page, Task } from "@/lib/types";

export default function ContactPage() {
  const { id } = useParams<{ id: string }>();
  const contact = useQuery({
    queryKey: keys.contact(id),
    queryFn: () => get<Contact>(`/contacts/${id}`),
  });
  return (
    <RecordView
      query={contact}
      missing={{
        title: "This contact no longer exists",
        href: "/contacts",
        back: "Back to contacts",
      }}
    >
      {(contact) => <ContactView contact={contact} />}
    </RecordView>
  );
}

function ContactView({ contact }: { contact: Contact }) {
  const me = useMe().data;
  const router = useRouter();
  const client = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [addingDeal, setAddingDeal] = useState(false);
  const [addingTask, setAddingTask] = useState(false);
  const editable = canModify(me, contact.owner_id);

  const remove = useMutation({
    mutationFn: () => del(`/contacts/${contact.id}`),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: ["contacts"] });
      router.push("/contacts");
    },
  });

  const tasks = useQuery({
    queryKey: keys.tasks({ contact: contact.id }),
    queryFn: () => get<Page<Task>>(`/tasks${query({ contact_id: contact.id, per_page: 50 })}`),
  });

  return (
    <>
      <Link
        href="/contacts"
        className="text-body text-muted hover:text-ink underline underline-offset-4"
      >
        Contacts
      </Link>
      <header className="mt-3 flex flex-wrap items-start justify-between gap-x-8 gap-y-4 pb-6">
        <div className="min-w-0">
          <h1 className="text-display font-semibold tracking-tight break-words">
            {fullName(contact)}
          </h1>
          <p className="text-muted mt-1">
            {contact.job_title ?? "No job title"}
            {contact.company_id ? (
              <>
                {" at "}
                <Link
                  href={`/companies/${contact.company_id}`}
                  className="hover:text-ink underline underline-offset-4"
                >
                  {contact.company_name}
                </Link>
              </>
            ) : null}
          </p>
          <p className="mt-3 flex flex-wrap gap-x-5 gap-y-1">
            {contact.email ? (
              <a
                href={`mailto:${contact.email}`}
                className="text-accent underline underline-offset-4"
              >
                {contact.email}
              </a>
            ) : null}
            {contact.phone ? (
              <a
                href={`tel:${contact.phone.replace(/\s/g, "")}`}
                className="num text-accent underline underline-offset-4"
              >
                {contact.phone}
              </a>
            ) : null}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="primary" onClick={() => setAddingDeal(true)}>
            New deal
          </Button>
          {editable ? <Button onClick={() => setEditing(true)}>Edit contact</Button> : null}
          {isManager(me) ? (
            <Button
              variant="danger"
              busy={remove.isPending}
              onClick={() => {
                if (
                  window.confirm(
                    `Delete ${fullName(contact)}? Their follow-ups and notes are deleted too.`,
                  )
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
            <div className="flex items-center justify-between gap-3 px-4 pt-4">
              <h2 className="text-title font-semibold">Follow-ups</h2>
              <Button variant="quiet" onClick={() => setAddingTask(true)}>
                Add follow-up
              </Button>
            </div>
            {tasks.isPending ? (
              <Loading label="Loading follow-ups…" />
            ) : tasks.isError ? (
              <ErrorState error={tasks.error} onRetry={() => tasks.refetch()} />
            ) : tasks.data.data.length === 0 ? (
              <p className="text-body text-muted px-4 py-6">No follow-ups with this person.</p>
            ) : (
              <ul className="divide-line border-line mt-2 divide-y border-t">
                {tasks.data.data.map((task) => (
                  <TaskRow key={task.id} task={task} showLinks={false} />
                ))}
              </ul>
            )}
          </Panel>
          <NotesPanel parent={{ contact_id: contact.id }} />
        </div>
        <div className="flex flex-col gap-6">
          <Panel>
            <h2 className="text-title px-4 pt-4 font-semibold">Deals</h2>
            <div className="border-line mt-2 border-t">
              <LinkedDeals filter={{ contact_id: contact.id }} />
            </div>
          </Panel>
          <ActivityPanel entityType="contact" entityId={contact.id} />
        </div>
      </div>

      <ContactFormDialog open={editing} onClose={() => setEditing(false)} contact={contact} />
      <DealFormDialog
        open={addingDeal}
        onClose={() => setAddingDeal(false)}
        defaults={{ contactId: contact.id, companyId: contact.company_id ?? undefined }}
      />
      <TaskFormDialog
        open={addingTask}
        onClose={() => setAddingTask(false)}
        link={{ contactId: contact.id }}
      />
    </>
  );
}
