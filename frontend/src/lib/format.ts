const moneyFormatters = new Map<string, Intl.NumberFormat>();

export function money(cents: number, currency: string, opts: { compact?: boolean } = {}): string {
  const key = `${currency}:${opts.compact ? "c" : "f"}`;
  let fmt = moneyFormatters.get(key);
  if (!fmt) {
    fmt = new Intl.NumberFormat("en-US", {
      style: "currency",
      currency,
      notation: opts.compact ? "compact" : "standard",
      maximumFractionDigits: opts.compact ? 1 : 0,
    });
    moneyFormatters.set(key, fmt);
  }
  return fmt.format(cents / 100);
}

export function percent(ratio: number | null): string {
  return ratio === null ? "n/a" : `${Math.round(ratio * 100)}%`;
}

const dateFmt = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" });
const dateYearFmt = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
  year: "numeric",
});
const timeFmt = new Intl.DateTimeFormat("en-US", { hour: "numeric", minute: "2-digit" });
const monthFmt = new Intl.DateTimeFormat("en-US", { month: "short", year: "numeric" });

export function parseDay(value: string): Date {
  const [y, m, d] = value.split("-").map(Number);
  return new Date(y ?? 1970, (m ?? 1) - 1, d ?? 1);
}

export function shortDate(value: string | Date): string {
  const date = typeof value === "string" ? toDate(value) : value;
  const sameYear = date.getFullYear() === new Date().getFullYear();
  return (sameYear ? dateFmt : dateYearFmt).format(date);
}

export function dateTime(value: string): string {
  const date = new Date(value);
  return `${shortDate(date)}, ${timeFmt.format(date)}`;
}

export function monthLabel(value: string): string {
  return monthFmt.format(parseDay(value));
}

function toDate(value: string): Date {
  return value.length === 10 ? parseDay(value) : new Date(value);
}

export function relativeDue(value: string, now = new Date()): string {
  const due = new Date(value);
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const days = Math.floor((due.getTime() - startOfToday.getTime()) / 86_400_000);
  if (days === 0) return `Today, ${timeFmt.format(due)}`;
  if (days === 1) return `Tomorrow, ${timeFmt.format(due)}`;
  if (days === -1) return "Yesterday";
  if (days < 0) return `${-days} days ago`;
  return shortDate(due);
}

export function isOverdue(task: { due_at: string | null; completed_at: string | null }): boolean {
  return !task.completed_at && !!task.due_at && new Date(task.due_at) < new Date();
}

export function fullName(c: { first_name: string; last_name: string }): string {
  return `${c.first_name} ${c.last_name}`.trim();
}

export function parseMoneyInput(raw: string): number | null {
  const cleaned = raw.replace(/[\s,]/g, "");
  if (cleaned === "") return 0;
  if (!/^\d+(\.\d{1,2})?$/.test(cleaned)) return null;
  return Math.round(Number(cleaned) * 100);
}

export function centsToInput(cents: number): string {
  return cents % 100 === 0 ? String(cents / 100) : (cents / 100).toFixed(2);
}

export const ROLE_LABELS: Record<string, string> = {
  admin: "Admin",
  manager: "Manager",
  sales_rep: "Sales rep",
};
