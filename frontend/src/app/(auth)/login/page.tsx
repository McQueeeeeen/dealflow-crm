import type { Metadata } from "next";
import { Suspense } from "react";

import { AuthForm } from "../auth-form";

export const metadata: Metadata = { title: "Sign in" };
export const dynamic = "force-dynamic";

export default function LoginPage() {
  const demo =
    process.env.DEMO_EMAIL && process.env.DEMO_PASSWORD
      ? { email: process.env.DEMO_EMAIL, password: process.env.DEMO_PASSWORD }
      : undefined;
  return (
    <Suspense>
      <AuthForm mode="login" demo={demo} />
    </Suspense>
  );
}
