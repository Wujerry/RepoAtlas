import type { ActivityDayRange, ActivityHistoryItem } from "../types";

export function localDate(date = new Date()): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
export function parseLocalDate(value: string): Date { return new Date(`${value}T00:00:00`); }
export function offsetDate(value: string, offset: number): string {
  const date = parseLocalDate(value); date.setDate(date.getDate() + offset); return localDate(date);
}
export function dayRange(date: string): ActivityDayRange {
  return { date, startAt: parseLocalDate(date).getTime() / 1000, endAt: parseLocalDate(offsetDate(date, 1)).getTime() / 1000 };
}
export function calendarDays(end: string): ActivityDayRange[] {
  return Array.from({ length: 30 }, (_, i) => dayRange(offsetDate(end, i - 29)));
}
export type FootprintRow = { type: "hour"; id: string; label: string } | { type: "item"; id: string; item: ActivityHistoryItem };
export function timelineRows(items: ActivityHistoryItem[], formatter: Intl.DateTimeFormat): FootprintRow[] {
  const rows: FootprintRow[] = []; let lastHour = "";
  for (const item of items) {
    const date = new Date(item.occurredAt);
    const hour = [date.getFullYear(), date.getMonth(), date.getDate(), date.getHours(), date.getTimezoneOffset()].join(":");
    if (hour !== lastHour) {
      rows.push({ type: "hour", id: `hour:${hour}`, label: formatter.format(date) }); lastHour = hour;
    }
    rows.push({ type: "item", id: item.id, item });
  }
  return rows;
}
