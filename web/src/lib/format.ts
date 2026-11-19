/** Dates, times and numbers, written one way across the app, in the reader's locale. */

const dateFmt = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });
const dateTimeFmt = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });
const timeFmt = new Intl.DateTimeFormat(undefined, { timeStyle: "short" });
const numberFmt = new Intl.NumberFormat();
const relativeFmt = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

type DateInput = string | number | Date;

function toDate(value: DateInput): Date | null {
  const d = value instanceof Date ? value : new Date(value);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** A person's full name: "First Last". */
export function formatName(user: { first_name: string; last_name: string }): string {
  return `${user.first_name} ${user.last_name}`.trim();
}

export function formatDate(value: DateInput | null | undefined): string {
  const d = value == null ? null : toDate(value);
  return d ? dateFmt.format(d) : "—";
}

export function formatDateTime(value: DateInput | null | undefined): string {
  const d = value == null ? null : toDate(value);
  return d ? dateTimeFmt.format(d) : "—";
}

export function formatTime(value: DateInput | null | undefined): string {
  const d = value == null ? null : toDate(value);
  return d ? timeFmt.format(d) : "—";
}

export function formatNumber(value: number | null | undefined): string {
  return value == null || Number.isNaN(value) ? "—" : numberFmt.format(value);
}

const STEPS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 3600],
  ["month", 30 * 24 * 3600],
  ["week", 7 * 24 * 3600],
  ["day", 24 * 3600],
  ["hour", 3600],
  ["minute", 60],
];

/** "3 hours ago", "in 2 days", "now". `now` is injectable for tests. */
export function formatRelative(
  value: DateInput | null | undefined,
  now: Date = new Date(),
): string {
  const d = value == null ? null : toDate(value);
  if (!d) return "—";
  const seconds = Math.round((d.getTime() - now.getTime()) / 1000);
  for (const [unit, size] of STEPS) {
    if (Math.abs(seconds) >= size) return relativeFmt.format(Math.round(seconds / size), unit);
  }
  return relativeFmt.format(0, "second");
}
