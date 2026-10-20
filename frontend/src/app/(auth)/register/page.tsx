import type { Metadata } from "next";
import { Suspense } from "react";

import { AuthForm } from "../auth-form";

export const metadata: Metadata = { title: "Create a workspace" };

export default function RegisterPage() {
  return (
    <Suspense>
      <AuthForm mode="register" />
    </Suspense>
  );
}
