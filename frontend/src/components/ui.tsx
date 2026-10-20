"use client";

import { IconX } from "@tabler/icons-react";
import {
  forwardRef,
  useEffect,
  useId,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from "react";

import Link from "next/link";

import { ApiError } from "@/lib/api";
import { useUnsavedChanges } from "@/lib/hooks";
import { BACK } from "@/lib/transitions";

export function cx(...classes: (string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(" ");
}

type Variant = "primary" | "secondary" | "quiet" | "danger";

const variants: Record<Variant, string> = {
  primary: "bg-accent text-accent-ink hover:brightness-110 border-transparent shadow-highlight",
  secondary: "bg-row text-ink border-line hover:bg-hover shadow-highlight",
  quiet: "bg-transparent text-ink border-transparent hover:bg-hover",
  danger: "bg-surface text-critical border-critical hover:bg-critical-soft",
};

export const Button = forwardRef<
  HTMLButtonElement,
  ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; busy?: boolean }
>(function Button({ variant = "secondary", busy, className, children, disabled, ...props }, ref) {
  return (
    <button
      ref={ref}
      className={cx(
        "rounded-control text-body inline-flex min-h-11 items-center justify-center gap-2 border px-4 font-medium transition-colors sm:min-h-9",
        "disabled:cursor-not-allowed disabled:opacity-55",
        variants[variant],
        className,
      )}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      {...props}
    >
      {busy ? "Saving…" : children}
    </button>
  );
});

const control =
  "w-full rounded-control border border-line bg-row px-3 text-body text-ink placeholder:text-muted min-h-11 sm:min-h-9 aria-[invalid=true]:border-critical";

export function Field({
  label,
  name,
  error,
  hint,
  children,
}: {
  label: string;
  name?: string;
  error?: string;
  hint?: string;
  children: (props: {
    id: string;
    name?: string;
    "aria-invalid"?: boolean;
    "aria-describedby"?: string;
  }) => ReactNode;
}) {
  const id = useId();
  const describedBy = error ? `${id}-error` : hint ? `${id}-hint` : undefined;
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-meta text-muted font-medium">
        {label}
      </label>
      {children({
        id,
        name,
        "aria-invalid": error ? true : undefined,
        "aria-describedby": describedBy,
      })}
      {error ? (
        <p id={`${id}-error`} className="text-meta text-critical">
          {error}
        </p>
      ) : hint ? (
        <p id={`${id}-hint`} className="text-meta text-muted">
          {hint}
        </p>
      ) : null}
    </div>
  );
}

export const TextInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function TextInput({ className, autoComplete = "off", spellCheck, ...props }, ref) {
    return (
      <input
        ref={ref}
        className={cx(control, className)}
        autoComplete={autoComplete}
        spellCheck={props.type === "email" ? false : spellCheck}
        {...props}
      />
    );
  },
);

export function Select({ className, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={cx(control, "pr-8", className)} {...props} />;
}

export function TextArea({ className, ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={cx(control, "min-h-24 py-2", className)} {...props} />;
}

export function Dialog({
  open,
  onClose,
  title,
  children,
  wide,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      onClose={onClose}
      aria-labelledby="dialog-title"
      className={cx(
        "rounded-dialog border-line bg-raised text-ink shadow-float m-auto w-[calc(100%-2rem)] border p-0",
        wide ? "max-w-2xl" : "max-w-lg",
      )}
    >
      {open ? (
        <div className="flex flex-col gap-5 p-5 sm:p-6">
          <div className="flex items-start justify-between gap-4">
            <h2 id="dialog-title" className="text-title font-semibold">
              {title}
            </h2>
            <button
              type="button"
              onClick={() => requestClose(ref.current, onClose)}
              className="rounded-control text-muted hover:bg-hover -mt-1 -mr-2 inline-flex size-11 items-center justify-center sm:size-9"
              aria-label="Close"
            >
              <IconX size={18} stroke={1.75} aria-hidden="true" />
            </button>
          </div>
          {children}
        </div>
      ) : null}
    </dialog>
  );
}

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <header className="flex flex-wrap items-end justify-between gap-x-6 gap-y-3 pb-6">
      <div className="min-w-0">
        <h1 className="text-display">{title}</h1>
        {description ? <p className="text-body text-muted mt-1">{description}</p> : null}
      </div>
      {actions ? <div className="flex flex-wrap gap-2">{actions}</div> : null}
    </header>
  );
}

export function Panel({
  className,
  quiet,
  children,
}: {
  className?: string;
  quiet?: boolean;
  children: ReactNode;
}) {
  return (
    <section
      className={cx(
        "border-line",
        quiet ? "border-t" : "rounded-card bg-surface shadow-highlight border",
        className,
      )}
    >
      {children}
    </section>
  );
}

type Tone = "open" | "won" | "lost" | "accent";

const tones: Record<Tone, string> = {
  open: "bg-open-soft text-open",
  won: "bg-won-soft text-won",
  lost: "bg-row text-muted",
  accent: "bg-accent-soft text-ink",
};

export function Pill({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span
      className={cx(
        "text-meta inline-flex h-6 items-center rounded-full px-2.5 font-medium whitespace-nowrap",
        tones[tone],
      )}
    >
      {children}
    </span>
  );
}

export function Stat({
  label,
  value,
  note,
}: {
  label: string;
  value: ReactNode;
  note?: ReactNode;
}) {
  return (
    <div className="rounded-card border-line bg-surface shadow-highlight flex min-w-0 flex-col gap-2 border p-4 sm:gap-3 sm:p-5">
      <p className="text-body text-muted font-medium">{label}</p>
      <p className="num text-title sm:text-display truncate font-semibold tracking-tight">{value}</p>
      {note ? <p className="text-meta text-muted">{note}</p> : null}
    </div>
  );
}

export function Loading({ label }: { label: string }) {
  return (
    <p role="status" className="text-body text-muted px-4 py-10 text-center">
      {label}
    </p>
  );
}

export function ErrorState({ error, onRetry }: { error: unknown; onRetry?: () => void }) {
  const message =
    error instanceof ApiError ? error.message : "Something went wrong while loading this view.";
  return (
    <div role="alert" className="flex flex-col items-center gap-3 px-4 py-10 text-center">
      <p className="text-body text-critical">{message}</p>
      {onRetry ? (
        <Button type="button" onClick={onRetry}>
          Try Again
        </Button>
      ) : null}
    </div>
  );
}

export function EmptyState({
  title,
  body,
  action,
}: {
  title: string;
  body?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center gap-2 px-4 py-12 text-center">
      <p className="text-lead font-medium">{title}</p>
      {body ? <p className="text-body text-muted max-w-md">{body}</p> : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}

export function FormError({ error }: { error: unknown }) {
  if (!error) return null;
  const message =
    error instanceof ApiError
      ? error.fields.length > 0 && !error.fields.some((f) => f.field === "body")
        ? "Some fields need attention."
        : error.message
      : "Could not save. Check your connection and try again.";
  return (
    <p role="alert" className="rounded-control bg-critical-soft text-body text-critical px-3 py-2">
      {message}
    </p>
  );
}

export function fieldError(error: unknown, field: string): string | undefined {
  return error instanceof ApiError ? error.fieldError(field) : undefined;
}

export function Pagination({
  page,
  perPage,
  total,
  onPage,
}: {
  page: number;
  perPage: number;
  total: number;
  onPage: (page: number) => void;
}) {
  const pages = Math.max(1, Math.ceil(total / perPage));
  if (pages <= 1) return null;
  return (
    <nav
      aria-label="Pagination"
      className="border-line flex items-center justify-between gap-3 border-t px-4 py-3"
    >
      <p className="num text-meta text-muted">
        Page {page} of {pages}, {total} total
      </p>
      <div className="flex gap-2">
        <Button type="button" variant="quiet" disabled={page <= 1} onClick={() => onPage(page - 1)}>
          Previous
        </Button>
        <Button
          type="button"
          variant="quiet"
          disabled={page >= pages}
          onClick={() => onPage(page + 1)}
        >
          Next
        </Button>
      </div>
    </nav>
  );
}

function requestClose(dialog: HTMLDialogElement | null | undefined, onClose: () => void) {
  if (!dialog || dialog.dispatchEvent(new Event("cancel", { cancelable: true }))) onClose();
}

export function ModalForm({
  onSubmit,
  onClose,
  error,
  busy,
  submitLabel,
  children,
}: {
  onSubmit: () => void;
  onClose: () => void;
  error: unknown;
  busy: boolean;
  submitLabel: string;
  children: ReactNode;
}) {
  const form = useRef<HTMLFormElement>(null);
  const [dirty, setDirty] = useState(false);
  useUnsavedChanges(dirty);

  useEffect(() => {
    const dialog = form.current?.closest("dialog");
    if (!dialog || !dirty) return;
    const guard = (e: Event) => {
      if (!window.confirm("Discard unsaved changes?")) e.preventDefault();
    };
    dialog.addEventListener("cancel", guard);
    return () => dialog.removeEventListener("cancel", guard);
  }, [dirty]);

  useEffect(() => {
    if (error) form.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
  }, [error]);

  return (
    <form
      ref={form}
      noValidate
      className="flex flex-col gap-4"
      onInput={() => setDirty(true)}
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit();
      }}
    >
      {children}
      <FormError error={error} />
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button
          type="button"
          variant="quiet"
          onClick={() => requestClose(form.current?.closest("dialog"), onClose)}
        >
          Cancel
        </Button>
        <Button type="submit" variant="primary" busy={busy}>
          {submitLabel}
        </Button>
      </div>
    </form>
  );
}

export function Missing({ title, href, back }: { title: string; href: string; back: string }) {
  return (
    <div className="py-12 text-center">
      <p className="text-lead font-medium">{title}</p>
      <Link
        transitionTypes={BACK}
        href={href}
        className="text-link mt-4 inline-block underline underline-offset-4"
      >
        {back}
      </Link>
    </div>
  );
}

export function RecordView<T>({
  query,
  missing,
  children,
}: {
  query: {
    isPending: boolean;
    isError: boolean;
    error: unknown;
    data: T | undefined;
    refetch: () => unknown;
  };
  missing: { title: string; href: string; back: string };
  children: (data: T) => ReactNode;
}) {
  if (query.isPending) return <Loading label="Loading…" />;
  if (query.error instanceof ApiError && query.error.status === 404)
    return <Missing {...missing} />;
  if (query.isError || query.data === undefined)
    return <ErrorState error={query.error} onRetry={() => query.refetch()} />;
  return <>{children(query.data)}</>;
}
