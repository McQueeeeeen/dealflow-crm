"use client";

import { useQuery } from "@tanstack/react-query";

import { get } from "./api";
import type { Me, Stage, User } from "./types";

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

export function canModify(me: Me | undefined, ownerId: string): boolean {
  return !!me && (me.role !== "sales_rep" || me.id === ownerId);
}

export function isManager(me: Me | undefined): boolean {
  return me?.role === "admin" || me?.role === "manager";
}
