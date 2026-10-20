"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { money } from "./format";
import { useMe } from "./queries";

export function useMoney() {
  const currency = useMe().data?.currency ?? "USD";
  return useCallback(
    (cents: number, opts?: { compact?: boolean }) => money(cents, currency, opts),
    [currency],
  );
}

export function useDebounced<T>(value: T, delay = 250): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(id);
  }, [value, delay]);
  return debounced;
}

export function useUnsavedChanges(active: boolean) {
  useEffect(() => {
    if (!active) return;
    const warn = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [active]);
}

export function useUrlParams() {
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();
  const update = useCallback(
    (patch: Record<string, string | number | null>) => {
      const search = new URLSearchParams(params.toString());
      for (const [key, value] of Object.entries(patch)) {
        if (value === null || value === "") search.delete(key);
        else search.set(key, String(value));
      }
      const qs = search.toString();
      router.replace(qs ? `${pathname}?${qs}` : pathname, { scroll: false });
    },
    [params, pathname, router],
  );
  return [params, update] as const;
}

export function useUrlSearch(q: string, update: (patch: Record<string, string | null>) => void) {
  const [search, setSearch] = useState(q);
  const debounced = useDebounced(search.trim());
  useEffect(() => {
    if (debounced !== q) update({ q: debounced, page: null });
  }, [debounced, q, update]);
  return [search, setSearch] as const;
}
