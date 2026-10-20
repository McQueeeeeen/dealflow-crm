import { Suspense, ViewTransition } from "react";

const MOTION = {
  "nav-forward": "nav-forward",
  "nav-back": "nav-back",
  "nav-section": "fade",
  default: "none",
};

export default function Template({ children }: { children: React.ReactNode }) {
  return (
    <ViewTransition enter={MOTION} exit={MOTION} default="none">
      <div>
        <Suspense>{children}</Suspense>
      </div>
    </ViewTransition>
  );
}
