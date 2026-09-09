import type { MessageKey } from "../i18n";
import type { UpdateErrorStage } from "../types";
import { formatLocaleTag } from "./format";

export function formatUpdateBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 1024) return `${Math.max(0, Math.round(bytes))} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`;
}

export function formatUpdateDate(value?: string, locale?: string): string | undefined {
  if (!value) return undefined;
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(formatLocaleTag(locale), { dateStyle: "medium", timeStyle: "short" }).format(parsed);
}

export function updateErrorTitle(stage: UpdateErrorStage | undefined, t: (key: MessageKey) => string): string {
  switch (stage) {
    case "download": return t("updateDownloadFailed");
    case "install": return t("updateInstallFailed");
    case "restart": return t("updateRestartFailed");
    default: return t("updateCheckFailed");
  }
}
