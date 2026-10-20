"use client";

import { useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { ViewTransition, type ReactNode } from "react";

import { keys } from "@/lib/queries";
import { FORWARD } from "@/lib/transitions";
import type { Deal } from "@/lib/types";

export function DealLink({
  deal,
  className,
  children,
}: {
  deal: Deal;
  className?: string;
  children: ReactNode;
}) {
  const client = useQueryClient();
  return (
    <Link
      href={`/deals/${deal.id}`}
      transitionTypes={FORWARD}
      className={className}
      onClick={() => client.setQueryData(keys.deal(deal.id), deal)}
    >
      {children}
    </Link>
  );
}

export function DealName({ deal, className }: { deal: Deal; className?: string }) {
  return (
    <ViewTransition name={`deal-${deal.id}`} share="morph" default="none">
      <span className={className}>{deal.title}</span>
    </ViewTransition>
  );
}
