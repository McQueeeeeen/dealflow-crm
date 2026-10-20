"use client";

import {
  forwardRef,
  useEffect,
  useId,
  useRef,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from "react";

import Link from "next/link";

import { ApiError } from "@/lib/api";

export function cx(...classes: (string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(" ");
}

type Variant = "primary" | "secondary" | "quiet" | "danger";

const variants: Record<Variant, string> = {
  primary: "bg-accent text-accent-ink hover:brightness-110 border-transparent",
  secondary: "bg-surface text-ink border-line-strong hover:bg-hover",
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
  "w-full rounded-control border border-line-strong bg-surface px-3 text-body text-ink placeholder:text-muted min-h-11 sm:min-h-9 aria-[invalid=true]:border-critical";

export function Field({
  label,
  error,
  hint,
  children,
}: {
  label: string;
  error?: string;
  hint?: string;
  children: (props: {
    id: string;
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
      {children({ id, "aria-invalid": error ? true : undefined, "aria-describedby": describedBy })}
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
  function TextInput({ className, ...props }, ref) {
    return <input ref={ref} className={cx(control, className)} {...props} />;
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
              onClick={onClose}
              className="rounded-control text-muted hover:bg-hover -mt-1 -mr-2 inline-flex size-11 items-center justify-center sm:size-9"
              aria-label="Close"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
                <path d="M2 2l10 10M12 2L2 12" stroke="currentColor" strokeWidth="1.6" />
              </svg>
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
        <h1 className="text-display font-semibold tracking-tight">{title}</h1>
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
        quiet ? "border-t" : "rounded-control bg-surface border",
        className,
      )}
    >
      {children}
    </section>
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
          Try again
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
  return (
    <form
      noValidate
      className="flex flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit();
      }}
    >
      {children}
      <FormError error={error} />
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button type="button" variant="quiet" onClick={onClose}>
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
      <Link href={href} className="text-accent mt-4 inline-block underline underline-offset-4">
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
