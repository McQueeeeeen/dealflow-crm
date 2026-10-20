import type { Stage, StageKind } from "./types";

type Oklch = [number, number, number];

const STOPS: Oklch[] = [
  [0.62, 0.16, 255],
  [0.7, 0.12, 200],
  [0.8, 0.15, 85],
  [0.7, 0.18, 50],
];

export const WON_HEAT = "oklch(0.63 0.21 32)";
export const LOST_HEAT = "var(--line-strong)";

export function heat(t: number): string {
  const x = Math.min(Math.max(t, 0), 1) * (STOPS.length - 1);
  const i = Math.min(Math.floor(x), STOPS.length - 2);
  const a = STOPS[i]!;
  const b = STOPS[i + 1]!;
  const mix = (k: 0 | 1 | 2) => (a[k] + (b[k] - a[k]) * (x - i)).toFixed(3);
  return `oklch(${mix(0)} ${mix(1)} ${mix(2)})`;
}

export function stageHeat(stages: Stage[], stage: { id: string; kind: StageKind }): string {
  if (stage.kind === "won") return WON_HEAT;
  if (stage.kind === "lost") return LOST_HEAT;
  const open = stages.filter((s) => s.kind === "open");
  const index = open.findIndex((s) => s.id === stage.id);
  return heat(open.length > 1 ? index / (open.length - 1) : 1);
}
