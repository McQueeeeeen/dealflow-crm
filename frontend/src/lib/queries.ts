"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

import { get, query } from "./api";
import type { Me, Page, Stage, Targets, Task, User } from "./types";

export const keys = {
  me: ["me"] as const,
  users: ["users"] as const,
  stages: ["stages"] as const,
  board: (filters: object) => ["board", filters] as const,
  deals: (filters: object) => ["deals", filters] as const,
  deal: (id: string) => ["deal", id] as const,
  stageHistory: (id: string) => ["deal", id, "stage-history"] as const,
  companies: (filters: object) => ["companies", filters] as const,
  company: (id: string) => ["company", id] as const,
  contacts: (filters: object) => ["contacts", filters] as const,
  contact: (id: string) => ["contact", id] as const,
  tasks: (filters: object) => ["tasks", filters] as const,
  notes: (filters: object) => ["notes", filters] as const,
  audit: (filters: object) => ["audit", filters] as const,
  analytics: (report: string, filters: object) => ["analytics", report, filters] as const,
};

const AFFECTS = {
  deal: ["board", "deals", "deal", "analytics", "audit", "targets", "companies", "automations"],
  task: ["tasks", "board", "deals", "deal", "audit"],
  note: ["notes", "board", "deals", "deal"],
  company: ["companies", "company", "deals", "board", "audit", "duplicates"],
  contact: ["contacts", "contact", "companies", "audit", "duplicates"],
  stage: ["stages", "board", "analytics", "audit"],
  user: ["users", "me", "audit"],
  target: ["targets"],
  view: ["views"],
  rule: ["automations", "audit"],
} satisfies Record<string, string[]>;

export type Entity = keyof typeof AFFECTS;

export function affected(...entities: string[]): string[] {
  return [...new Set(entities.flatMap((e) => AFFECTS[e as Entity] ?? []))];
}

export function useInvalidate() {
  const client = useQueryClient();
  return useCallback(
    (...entities: Entity[]) => {
      for (const key of affected(...entities)) client.invalidateQueries({ queryKey: [key] });
    },
    [client],
  );
}

export function useMe() {
  return useQuery({ queryKey: keys.me, queryFn: () => get<Me>("/auth/me"), staleTime: 60_000 });
}

export function useUsers() {
  return useQuery({
    queryKey: keys.users,
    queryFn: () => get<User[]>("/users"),
    staleTime: 60_000,
  });
}

export function useStages() {
  return useQuery({
    queryKey: keys.stages,
    queryFn: () => get<Stage[]>("/stages"),
    staleTime: 60_000,
  });
}

export function useMyTasks() {
  const me = useMe().data;
  return useQuery({
    queryKey: keys.tasks({ assignee: me?.id, status: "open", view: "today" }),
    queryFn: () =>
      get<Page<Task>>(`/tasks${query({ assignee_id: me?.id, status: "open", per_page: 100 })}`),
    enabled: !!me,
  });
}

export function useTargets(month?: string) {
  return useQuery({
    queryKey: ["targets", month ?? "current"],
    queryFn: () => get<Targets>(`/targets${query({ month })}`),
  });
}

export function dueToday(tasks: Task[], now = new Date()): number {
  const end = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  return tasks.filter((t) => t.due_at && new Date(t.due_at) < end).length;
}

export function canModify(me: Me | undefined, ownerId: string): boolean {
  return !!me && (me.role !== "sales_rep" || me.id === ownerId);
}

export function isManager(me: Me | undefined): boolean {
  return me?.role === "admin" || me?.role === "manager";
}
