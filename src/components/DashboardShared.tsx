import type { MessageKey } from "../i18n";
import type { ProjectDetail } from "../types";
import { Button } from "./ui/button";
export { MarkdownDocument } from "./MarkdownDocument";

export function InlineLoadError({ title, detail, retryLabel, onRetry }: { title: string; detail?: string; retryLabel: string; onRetry?: () => void }) {
  return <div className="inline-load-error" role="alert"><div><strong>{title}</strong>{detail && <code>{detail}</code>}</div>{onRetry && <Button onClick={onRetry}>{retryLabel}</Button>}</div>;
}

export function taskConfirmation(task: ProjectDetail["tasks"][number] | undefined, projectPath: string, t: (key: MessageKey) => string) {
  if (!task) return "";
  const command = formatCommand(task.executable, task.argv);
  return `${t("taskCommand")}: ${command}\n${t("workingDirectory")}: ${task.cwd ?? projectPath}\n\n${t("confirmTaskRunHint")}`;
}

export function formatCommand(executable: string, argv: string[]) {
  return [executable, ...argv].map((part) => /[\s"']/u.test(part) ? JSON.stringify(part) : part).join(" ");
}

export function statusKey(status: string): MessageKey {
  return status === "succeeded" ? "succeeded" : status === "failed" ? "failed" : status === "cancelled" ? "cancelled" : "running";
}
