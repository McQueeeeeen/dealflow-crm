"use client";

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
