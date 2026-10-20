"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Button, Field, FormError, TextInput, fieldError } from "@/components/ui";
import { ApiError } from "@/lib/api";

type Mode = "login" | "register";

interface Props {
  mode: Mode;
  demo?: { email: string; password: string };
}

function safeNext(raw: string | null): string {
  return raw && raw.startsWith("/") && !raw.startsWith("//") ? raw : "/today";
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
      <div className="max-w-md">
        <h1 className="text-display font-semibold tracking-tight">
          {isLogin ? "Sign in to your pipeline" : "Set up a workspace for your team"}
        </h1>
        <p className="text-lead text-muted mt-3">
          {isLogin
            ? "Pick up where you left off: today's follow-ups, deals in motion and what is likely to close this month."
            : "You become the workspace admin. Invite your team from Settings once you are in."}
        </p>
      </div>

      <form
        onSubmit={submit}
        noValidate
        className="rounded-control border-line bg-surface flex flex-col gap-4 border p-5 sm:p-6"
      >
        {!isLogin ? (
          <>
            <Field label="Company name" error={fieldError(error, "organization_name")}>
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
            <Field label="Your name" error={fieldError(error, "full_name")}>
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
        <Field label="Work email" error={fieldError(error, "email")}>
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
          {isLogin ? "Sign in" : "Create workspace"}
        </Button>

        {isLogin && demo ? (
          <Button
            type="button"
            variant="quiet"
            onClick={() => setValues((v) => ({ ...v, email: demo.email, password: demo.password }))}
          >
            Fill in the demo account
          </Button>
        ) : null}

        <p className="text-body text-muted text-center">
          {isLogin ? "New team? " : "Already have an account? "}
          <Link
            href={isLogin ? "/register" : "/login"}
            className="text-accent font-medium underline underline-offset-4"
          >
            {isLogin ? "Create a workspace" : "Sign in"}
          </Link>
        </p>
      </form>
    </div>
  );
}
