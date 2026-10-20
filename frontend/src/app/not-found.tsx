import Link from "next/link";
import { SECTION } from "@/lib/transitions";

export default function NotFound() {
  return (
    <main className="mx-auto flex min-h-dvh max-w-md flex-col justify-center gap-3 px-4">
      <h1 className="text-display font-semibold">This Page Does Not Exist</h1>
      <p className="text-muted">
        The link may be out of date, or the record was deleted by someone on your team.
      </p>
      <Link
        transitionTypes={SECTION}
        href="/today"
        className="text-accent font-medium underline underline-offset-4"
      >
        Back to today&apos;s work
      </Link>
    </main>
  );
}
