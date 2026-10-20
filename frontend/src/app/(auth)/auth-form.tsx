"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Button, Field, FormError, TextInput, fieldError, cx } from "@/components/ui";
import { ApiError } from "@/lib/api";
import { WON_HEAT, heat } from "@/lib/heat";

type Mode = "login" | "register";

const STAGES = ["Lead", "Qualified", "Proposal", "Negotiation", "Won"];

interface Props {
  mode: Mode;
  demo?: { email: string; password: string };
}

function safeNext(raw: string | null): string {
  return raw && /^\/(?![/\\])/.test(raw) ? raw : "/today";
}

export function AuthForm({ mode, demo }: Props) {
  const params = useSearchParams();
  const [values, setValues] = useState({
    organization_name: "",
    full_name: "",
    email: "",
    password: "",
  });
  const [error, setError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);

  const set = (key: keyof typeof values) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    const body = mode === "login" ? { email: values.email, password: values.password } : values;
    try {
      const res = await fetch(`/api/auth/${mode}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!res.ok) {
        const payload = await res.json().catch(() => null);
        const err = payload?.error;
        throw new ApiError(
          res.status,
          err?.code ?? "unknown",
          err?.message ?? "Request failed",
          err?.fields ?? [],
        );
      }
      window.location.assign(safeNext(params.get("next")));
    } catch (err) {
      setError(err);
      setBusy(false);
    }
  }

  const isLogin = mode === "login";

  return (
    <div className="grid gap-10 md:grid-cols-[1fr_minmax(0,26rem)] md:items-center">
      <div className="max-w-xl">
        <h1 className="text-display sm:text-hero font-semibold">
          {isLogin ? "Every Deal, from Cold Lead to Signed." : "Set Up a Workspace for Your Team"}
        </h1>
        <p className="text-lead text-muted mt-4 max-w-md">
          {isLogin
            ? "Pick up where you left off: today's follow-ups, deals in motion and what is likely to close this month."
            : "You become the workspace admin. Invite your team from Settings once you are in."}
        </p>
        <ol aria-label="Pipeline Stages" className="mt-10 grid grid-cols-5 gap-1.5">
          {STAGES.map((stage, i) => {
            const last = i === STAGES.length - 1;
            return (
              <li key={stage} className="min-w-0">
                <span aria-hidden="true" className="flex gap-1">
                  {[0, 1, 2, 3].map((t) => (
                    <span
                      key={t}
                      className="rail-fill block h-9 w-1.5 rounded-full"
                      style={{
                        animationDelay: `${(i * 4 + t) * 35}ms`,
                        backgroundColor: last ? WON_HEAT : heat(i / (STAGES.length - 2)),
                      }}
                    />
                  ))}
                </span>
                <span
                  className={cx(
                    "text-meta mt-2 block truncate",
                    last ? "text-won font-medium" : "text-muted",
                  )}
                >
                  {stage}
                </span>
              </li>
            );
          })}
        </ol>
      </div>

      <form
        onSubmit={submit}
        noValidate
        className="rounded-card border-line bg-surface shadow-highlight flex flex-col gap-4 border p-5 sm:p-6"
      >
        {!isLogin ? (
          <>
            <Field
              label="Company name"
              name="organization_name"
              error={fieldError(error, "organization_name")}
            >
              {(p) => (
                <TextInput
                  {...p}
                  value={values.organization_name}
                  onChange={set("organization_name")}
                  autoComplete="organization"
                  required
                />
              )}
            </Field>
            <Field label="Your name" name="full_name" error={fieldError(error, "full_name")}>
              {(p) => (
                <TextInput
                  {...p}
                  value={values.full_name}
                  onChange={set("full_name")}
                  autoComplete="name"
                  required
                />
              )}
            </Field>
          </>
        ) : null}
        <Field label="Work email" name="email" error={fieldError(error, "email")}>
          {(p) => (
            <TextInput
              {...p}
              type="email"
              value={values.email}
              onChange={set("email")}
              autoComplete="email"
              required
            />
          )}
        </Field>
        <Field
          label="Password"
          name="password"
          error={fieldError(error, "password")}
          hint={isLogin ? undefined : "At least 10 characters."}
        >
          {(p) => (
            <TextInput
              {...p}
              type="password"
              value={values.password}
              onChange={set("password")}
              autoComplete={isLogin ? "current-password" : "new-password"}
              required
            />
          )}
        </Field>

        <FormError error={error instanceof ApiError && error.fields.length > 0 ? null : error} />

        <Button type="submit" variant="primary" busy={busy}>
          {isLogin ? "Sign In" : "Create Workspace"}
        </Button>

        {isLogin && demo ? (
          <Button
            type="button"
            variant="quiet"
            onClick={() => setValues((v) => ({ ...v, email: demo.email, password: demo.password }))}
          >
            Fill In the Demo Account
          </Button>
        ) : null}

        <p className="text-body text-muted text-center">
          {isLogin ? "New team? " : "Already have an account? "}
          <Link
            href={isLogin ? "/register" : "/login"}
            className="text-link font-medium underline underline-offset-4"
          >
            {isLogin ? "Create a workspace" : "Sign In"}
          </Link>
        </p>
      </form>
    </div>
  );
}
