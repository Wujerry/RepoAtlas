export function formatTime(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export function stackOf(project: { languages: string[]; frameworks: string[]; packageManagers: string[] }): string[] {
  return [...project.languages, ...project.frameworks, ...project.packageManagers].slice(0, 6);
}
