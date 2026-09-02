let activeFormatLocale: "zh" | "en" = "en";

export function setFormatLocale(locale: string): void {
  activeFormatLocale = locale === "zh" ? "zh" : "en";
}

export function formatLocaleTag(locale?: string): string {
  const resolved = locale ?? activeFormatLocale;
  return resolved === "zh" ? "zh-CN" : "en-US";
}

export function formatTime(value?: string | null, locale?: string): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(formatLocaleTag(locale), {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export function stackOf(project: { languages: string[]; frameworks: string[]; packageManagers: string[] }): string[] {
  return [...project.languages, ...project.frameworks, ...project.packageManagers].slice(0, 6);
}
