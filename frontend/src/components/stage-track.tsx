import { cx } from "@/components/ui";
import { stageHeat } from "@/lib/heat";
import type { Stage, StageKind } from "@/lib/types";

export function StageTrack({
  stages,
  stageId,
  kind,
  size = "sm",
}: {
  stages: Stage[];
  stageId: string;
  kind: StageKind;
  size?: "sm" | "lg";
}) {
  const open = stages.filter((s) => s.kind === "open");
  const index = open.findIndex((s) => s.id === stageId);
  const filled = kind === "won" ? open.length : index + 1;
  const current = stages.find((s) => s.id === stageId);
  const label =
    kind === "open" && index >= 0
      ? `Stage ${index + 1} of ${open.length}: ${current?.name ?? ""}`
      : kind === "won"
        ? "Won"
        : "Lost";

  return (
    <span
      role="img"
      aria-label={label}
      className={cx("inline-flex shrink-0", size === "lg" ? "gap-1" : "gap-0.5")}
    >
      {open.map((stage, i) => (
        <span
          key={stage.id}
          className={cx(
            "rounded-full",
            size === "lg" ? "h-2 w-10 sm:w-14" : "h-1.5 w-3.5",
            (kind === "lost" || i >= filled) && "bg-track",
          )}
          style={
            kind !== "lost" && i < filled ? { backgroundColor: stageHeat(stages, stage) } : undefined
          }
        />
      ))}
    </span>
  );
}
