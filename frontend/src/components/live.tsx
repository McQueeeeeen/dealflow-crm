"use client";

import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { usePathname } from "next/navigation";
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";

import { Avatar, cx } from "@/components/ui";
import { put } from "@/lib/api";
import type { Viewer } from "@/lib/types";

interface Change {
  entity: string;
  id: string;
  actor: string | null;
}

interface Live {
  connected: boolean;
  viewers: Viewer[];
}

const LiveContext = createContext<Live>({ connected: false, viewers: [] });

const AFFECTS: Record<string, string[][]> = {
  deal: [["board"], ["deals"], ["deal"], ["analytics"], ["audit"], ["targets"]],
  task: [["tasks"], ["board"], ["deals"], ["deal"]],
  note: [["notes"], ["board"], ["deals"], ["deal"]],
  company: [["companies"], ["company"], ["deals"], ["board"]],
  contact: [["contacts"], ["contact"], ["companies"]],
  stage: [["stages"], ["board"], ["analytics"]],
  user: [["users"], ["me"]],
  target: [["targets"]],
};

function batcher(client: QueryClient) {
  const pending = new Set<string>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  return (keys: string[][]) => {
    for (const key of keys) pending.add(JSON.stringify(key));
    clearTimeout(timer);
    timer = setTimeout(() => {
      for (const key of pending) client.invalidateQueries({ queryKey: JSON.parse(key) });
      pending.clear();
    }, 250);
  };
}

function announce(connection: string | null, view: string) {
  if (connection) put("/live/presence", { connection, view }).catch(() => null);
}

export function LiveProvider({ me, children }: { me: string; children: ReactNode }) {
  const client = useQueryClient();
  const pathname = usePathname();
  const [state, setState] = useState<Live>({ connected: false, viewers: [] });
  const connection = useRef<string | null>(null);
  const view = useRef(pathname);

  useEffect(() => {
    const invalidate = batcher(client);
    let source: EventSource | null = null;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let attempt = 0;

    function open() {
      source = new EventSource("/api/v1/live");
      source.addEventListener("hello", (e) => {
        attempt = 0;
        connection.current = (e as MessageEvent<string>).data;
        setState((s) => ({ ...s, connected: true }));
        announce(connection.current, view.current);
      });
      source.addEventListener("change", (e) => {
        const change: Change = JSON.parse((e as MessageEvent<string>).data);
        if (change.actor !== me) invalidate(AFFECTS[change.entity] ?? []);
      });
      source.addEventListener("presence", (e) => {
        const viewers: Viewer[] = JSON.parse((e as MessageEvent<string>).data);
        setState((s) => ({ ...s, viewers }));
      });
      source.addEventListener("resync", () => client.invalidateQueries());
      source.onerror = () => {
        setState({ connected: false, viewers: [] });
        if (source?.readyState !== EventSource.CLOSED) return;
        attempt += 1;
        retry = setTimeout(
          async () => {
            await fetch("/api/auth/refresh", { method: "POST" }).catch(() => null);
            open();
          },
          Math.min(30_000, 1000 * 2 ** attempt),
        );
      };
    }

    open();
    return () => {
      clearTimeout(retry);
      source?.close();
    };
  }, [client, me]);

  useEffect(() => {
    view.current = pathname;
    announce(connection.current, pathname);
  }, [pathname]);

  return <LiveContext value={state}>{children}</LiveContext>;
}

export function LiveStatus({ me, compact }: { me: string; compact?: boolean }) {
  const { connected, viewers } = useContext(LiveContext);
  const pathname = usePathname();
  const here = viewers.filter(
    (v, i, all) =>
      v.view === pathname &&
      v.user_id !== me &&
      all.findIndex((o) => o.user_id === v.user_id && o.view === v.view) === i,
  );
  return (
    <div className="text-meta text-muted ml-auto flex items-center gap-3">
      {here.length > 0 ? (
        <span className="flex items-center gap-2">
          <span className="flex -space-x-1.5">
            {here.slice(0, 4).map((v) => (
              <span key={v.user_id} title={v.name} className="ring-canvas rounded-full ring-2">
                <Avatar name={v.name} />
              </span>
            ))}
          </span>
          <span className={cx(compact && "sr-only")}>
            {here.length === 1 ? `${here[0]!.name} is here too` : `${here.length} teammates here`}
          </span>
        </span>
      ) : null}
      <span className="flex items-center gap-1.5" role="status">
        <span
          aria-hidden="true"
          className={
            connected ? "bg-won size-2 rounded-full" : "bg-line-strong size-2 rounded-full"
          }
        />
        <span className={cx(compact && "sr-only")}>{connected ? "Live" : "Offline"}</span>
      </span>
    </div>
  );
}
