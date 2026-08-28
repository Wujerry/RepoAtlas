import { openUrl } from "@tauri-apps/plugin-opener";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { MessageKey } from "../i18n";
import type { ProjectDetail } from "../types";
import { Button } from "./ui/button";

export function MarkdownDocument({ content }: { content: string }) {
  return <article className="readme-document"><ReactMarkdown remarkPlugins={[remarkGfm]} components={{
    a: ({ href, children }) => {
      const external = Boolean(href && /^https?:\/\//i.test(href));
      return <a href={href} onClick={external ? (event) => { event.preventDefault(); void openUrl(href!); } : undefined}>{children}</a>;
    },
    img: ({ alt }) => <span className="readme-image-placeholder">{alt || "Image"}</span>,
  }}>{content}</ReactMarkdown></article>;
}

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
