"use client";

import { IconAlertTriangle, IconCircleCheck } from "@tabler/icons-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import Link from "next/link";
import { useState } from "react";

import { RuleDialog, actionText, triggerText } from "@/components/rule-form";
import {
  Button,
  EmptyState,
  ErrorState,
  Loading,
  PageHeader,
  Panel,
  Pill,
  cx,
} from "@/components/ui";
import { del, get, put } from "@/lib/api";
import { dateTime } from "@/lib/format";
import { isManager, useInvalidate, useMe, useStages, useUsers } from "@/lib/queries";
import { FORWARD } from "@/lib/transitions";
import type { Rule, RuleRun } from "@/lib/types";

export default function AutomationsPage() {
  const me = useMe().data;
  const manager = isManager(me);
  const [editing, setEditing] = useState<Rule | "new" | null>(null);
  const rules = useQuery({ queryKey: ["automations"], queryFn: () => get<Rule[]>("/automations") });

  return (
    <>
      <PageHeader
        title="Automations"
        description="Rules run the moment a deal changes, inside the same save. A rule that fails is undone on its own and listed under recent runs; the change still goes through."
        actions={
          manager ? (
            <Button variant="primary" onClick={() => setEditing("new")}>
              New Automation
            </Button>
          ) : null
        }
      />
      <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <section aria-label="Rules" className="flex flex-col gap-3">
          {rules.isPending ? (
            <Loading label="Loading automations…" />
          ) : rules.isError ? (
            <ErrorState error={rules.error} onRetry={() => rules.refetch()} />
          ) : rules.data.length === 0 ? (
            <Panel>
              <EmptyState
                title="No automations yet"
                body="Create a follow-up whenever a big deal reaches Proposal, or note who to call when a deal is won."
                action={
                  manager ? (
                    <Button onClick={() => setEditing("new")}>New Automation</Button>
                  ) : undefined
                }
              />
            </Panel>
          ) : (
            rules.data.map((rule) => (
              <RuleCard
                key={rule.id}
                rule={rule}
                editable={manager}
                onEdit={() => setEditing(rule)}
              />
            ))
          )}
        </section>
        <RunLog />
      </div>
      <RuleDialog rule={editing} onClose={() => setEditing(null)} />
    </>
  );
}

function RuleCard({
  rule,
  editable,
  onEdit,
}: {
  rule: Rule;
  editable: boolean;
  onEdit: () => void;
}) {
  const stages = useStages().data ?? [];
  const users = useUsers().data ?? [];
  const invalidate = useInvalidate();
  const toggle = useMutation({
    mutationFn: () => put<Rule>(`/automations/${rule.id}`, { ...rule, enabled: !rule.enabled }),
    onSuccess: () => invalidate("rule"),
  });
  const remove = useMutation({
    mutationFn: () => del(`/automations/${rule.id}`),
    onSuccess: () => invalidate("rule"),
  });

  return (
    <Panel className={cx("p-4", !rule.enabled && "opacity-70")}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <h2 className="text-lead font-semibold">{rule.name}</h2>
          {rule.enabled ? null : <Pill tone="lost">Paused</Pill>}
        </div>
        {editable ? (
          <div className="flex items-center gap-1">
            <button
              type="button"
              role="switch"
              aria-checked={rule.enabled}
              aria-label={`Run “${rule.name}”`}
              disabled={toggle.isPending}
              onClick={() => toggle.mutate()}
              className={cx(
                "relative mr-2 inline-flex h-6 w-10 shrink-0 items-center rounded-full transition-colors",
                rule.enabled ? "bg-accent" : "bg-track",
              )}
            >
              <span
                aria-hidden="true"
                className={cx(
                  "size-4 rounded-full bg-white transition-transform",
                  rule.enabled ? "translate-x-5" : "translate-x-1",
                )}
              />
            </button>
            <Button variant="quiet" onClick={onEdit}>
              Edit
            </Button>
            <Button
              variant="quiet"
              busy={remove.isPending}
              onClick={() => {
                if (window.confirm(`Delete the automation “${rule.name}”?`)) remove.mutate();
              }}
            >
              Delete
            </Button>
          </div>
        ) : null}
      </div>
      <p className="text-body mt-2">
        <span className="text-muted">When </span>
        {triggerText(rule.trigger, stages)}
        {rule.condition ? (
          <>
            <span className="text-muted"> and the deal matches </span>
            <code className="bg-row rounded px-1.5 py-0.5 font-mono text-[0.8125rem]">
              {rule.condition}
            </code>
          </>
        ) : null}
      </p>
      <ol className="text-body mt-2 flex flex-col gap-1">
        {rule.actions.map((action, i) => (
          <li key={i} className="flex gap-2">
            <span className="num text-muted shrink-0">{i + 1}.</span>
            <span className="min-w-0 break-words">{actionText(action, users)}</span>
          </li>
        ))}
      </ol>
      <p className="text-meta text-muted mt-3">
        <span className="num">{rule.runs_7d}</span> {rule.runs_7d === 1 ? "run" : "runs"} in the
        last 7 days
        {rule.failures_7d > 0 ? (
          <span className="text-critical">
            , <span className="num">{rule.failures_7d}</span> failed
          </span>
        ) : null}
        {rule.last_run_at ? `, last at ${dateTime(rule.last_run_at)}` : ""}
      </p>
    </Panel>
  );
}

function RunLog() {
  const runs = useQuery({
    queryKey: ["automations", "runs"],
    queryFn: () => get<RuleRun[]>("/automations/runs"),
  });
  return (
    <Panel>
      <h2 className="text-title px-4 pt-4 font-semibold">Recent Runs</h2>
      {runs.isPending ? (
        <Loading label="Loading runs…" />
      ) : runs.isError ? (
        <ErrorState error={runs.error} onRetry={() => runs.refetch()} />
      ) : runs.data.length === 0 ? (
        <p className="text-body text-muted px-4 py-6">
          Runs appear here as deals move. Only deals that match a rule are listed.
        </p>
      ) : (
        <ul className="row-list mt-1">
          {runs.data.map((run) => (
            <li key={run.id} className="flex gap-3 px-3.5 py-3">
              {run.succeeded ? (
                <IconCircleCheck size={18} stroke={1.75} className="text-won mt-0.5 shrink-0" aria-label="Done" />
              ) : (
                <IconAlertTriangle
                  size={18}
                  stroke={1.75}
                  className="text-critical mt-0.5 shrink-0"
                  aria-label="Failed"
                />
              )}
              <div className="min-w-0 flex-1">
                <p className="text-body">
                  <span className="font-medium">{run.rule_name}</span>
                  <span className="text-muted"> on </span>
                  <Link
                    transitionTypes={FORWARD}
                    href={`/deals/${run.deal_id}`}
                    className="underline underline-offset-2"
                  >
                    {run.deal_title}
                  </Link>
                </p>
                <p className={cx("text-meta mt-0.5", run.succeeded ? "text-muted" : "text-critical")}>
                  {run.detail}
                </p>
              </div>
              <time className="num text-meta text-muted shrink-0" dateTime={run.created_at}>
                {dateTime(run.created_at)}
              </time>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
