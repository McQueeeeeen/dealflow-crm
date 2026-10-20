"use client";

import { useQuery } from "@tanstack/react-query";

import { DealLink, DealName } from "@/components/deal-link";
import { StageTrack } from "@/components/stage-track";
import { ErrorState, Loading } from "@/components/ui";
import { get, query } from "@/lib/api";
import { shortDate } from "@/lib/format";
import { useMoney } from "@/lib/hooks";
import { keys, useStages } from "@/lib/queries";
import type { Deal, Page } from "@/lib/types";

export function LinkedDeals({
  filter,
}: {
  filter: { company_id: string } | { contact_id: string };
}) {
  const stages = useStages().data ?? [];
  const fmt = useMoney();
  const deals = useQuery({
    queryKey: keys.deals({ ...filter, linked: true }),
    queryFn: () => get<Page<Deal>>(`/deals${query({ ...filter, per_page: 100 })}`),
  });

  if (deals.isPending) return <Loading label="Loading deals…" />;
  if (deals.isError) return <ErrorState error={deals.error} onRetry={() => deals.refetch()} />;
  if (deals.data.data.length === 0) {
    return <p className="text-body text-muted px-4 py-6">No deals linked yet.</p>;
  }
  const rank = { open: 0, won: 1, lost: 2 };
  const sorted = [...deals.data.data].sort((a, b) => rank[a.stage_kind] - rank[b.stage_kind]);

  return (
    <ul className="divide-line divide-y">
      {sorted.map((deal) => (
        <li key={deal.id}>
          <DealLink deal={deal} className="hover:bg-hover flex items-center gap-3 px-4 py-3">
            <div className="min-w-0 flex-1">
              <DealName deal={deal} className="block truncate font-medium" />
              <p className="text-meta text-muted mt-1 flex items-center gap-2">
                <StageTrack stages={stages} stageId={deal.stage_id} kind={deal.stage_kind} />
                {deal.stage_name}
                {deal.closed_at ? `, ${shortDate(deal.closed_at)}` : ""}
              </p>
            </div>
            <span className="num shrink-0 font-medium">{fmt(deal.value_cents)}</span>
          </DealLink>
        </li>
      ))}
    </ul>
  );
}
