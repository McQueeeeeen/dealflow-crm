"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { Button, ErrorState, FormError, Loading, Panel, TextArea } from "@/components/ui";
import { del, get, post, query } from "@/lib/api";
import { dateTime } from "@/lib/format";
import { canModify, keys, useMe } from "@/lib/queries";
import type { Note, Page } from "@/lib/types";

type Parent = { deal_id: string } | { contact_id: string } | { company_id: string };

export function NotesPanel({ parent }: { parent: Parent }) {
  const me = useMe().data;
  const client = useQueryClient();
  const [body, setBody] = useState("");
  const notesKey = keys.notes(parent);

  const notes = useQuery({
    queryKey: notesKey,
    queryFn: () => get<Page<Note>>(`/notes${query({ ...parent, per_page: 50 })}`),
  });
  const add = useMutation({
    mutationFn: () => post<Note>("/notes", { body, ...parent }),
    onSuccess: () => {
      setBody("");
      client.invalidateQueries({ queryKey: notesKey });
    },
  });
  const remove = useMutation({
    mutationFn: (id: string) => del(`/notes/${id}`),
    onSuccess: () => client.invalidateQueries({ queryKey: notesKey }),
  });

  function submit(e: { preventDefault: () => void }) {
    e.preventDefault();
    if (body.trim()) add.mutate();
  }

  return (
    <Panel>
      <h2 className="text-title px-4 pt-4 font-semibold">Notes</h2>
      <form onSubmit={submit} className="border-line flex flex-col gap-2 border-b p-4">
        <label htmlFor="note-body" className="sr-only">
          New note
        </label>
        <TextArea
          id="note-body"
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder="What did you learn on the last call?"
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) submit(e);
          }}
        />
        <FormError error={add.error} />
        <div className="flex items-center justify-between gap-3">
          <span className="text-meta text-muted hidden sm:inline">Ctrl or Cmd + Enter to save</span>
          <Button type="submit" busy={add.isPending} disabled={!body.trim()}>
            Save note
          </Button>
        </div>
      </form>
      {notes.isPending ? (
        <Loading label="Loading notes…" />
      ) : notes.isError ? (
        <ErrorState error={notes.error} onRetry={() => notes.refetch()} />
      ) : notes.data.data.length === 0 ? (
        <p className="text-body text-muted px-4 py-6">
          No notes yet. Notes are shared with everyone in your workspace.
        </p>
      ) : (
        <ul className="divide-line divide-y">
          {notes.data.data.map((note) => (
            <li key={note.id} className="px-4 py-3">
              <p className="break-words whitespace-pre-wrap">{note.body}</p>
              <p className="text-meta text-muted mt-1 flex items-center gap-3">
                <span>{note.author_name}</span>
                <span className="num">{dateTime(note.created_at)}</span>
                {canModify(me, note.author_id) ? (
                  <button
                    type="button"
                    className="rounded-control hover:bg-critical-soft hover:text-critical ml-auto min-h-11 px-2 sm:min-h-7"
                    onClick={() => {
                      if (window.confirm("Delete this note?")) remove.mutate(note.id);
                    }}
                  >
                    Delete
                  </button>
                ) : null}
              </p>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
