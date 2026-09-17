import { Dialog } from "@base-ui/react/dialog";
import { ArrowsInSimple, ArrowsOutSimple, ArrowSquareOut, Code, Copy, FileCode, FolderOpen, X } from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import type { ProjectFile, ReadmeDocument, ToastTone } from "../types";
import { CodePreview } from "./CodePreview";
import { InlineLoadError } from "./DashboardShared";
import { Button } from "./ui/button";
import { Skeleton } from "./ui/feedback";

export function evidenceLanguage(path: string) {
  const name = path.split("\\").join("/").split("/").pop()?.toLowerCase() ?? "";
  if (name === "cargo.lock") return "toml";
  if (name === "composer.lock" || name === "package-lock.json") return "json";
  if (name === "gemfile" || name === "gemfile.lock") return "ruby";
  const extension = name.split(".").pop() ?? "";
  return ({ json: "json", toml: "toml", yaml: "yaml", yml: "yaml", xml: "xml", props: "xml", csproj: "xml", gradle: "groovy", kts: "kotlin", js: "javascript", ts: "typescript" } as Record<string, string>)[extension] ?? "text";
}

export function ProjectFileDialog({ projectId, file, onClose, notify, t }: {
  projectId: string;
  file: ProjectFile;
  onClose: () => void;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  t: (key: MessageKey) => string;
}) {
  const [document, setDocument] = useState<ReadmeDocument>();
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);
  const [expanded, setExpanded] = useState(false);
  const [busy, setBusy] = useState<string>();
  const language = evidenceLanguage(file.path);
  const name = file.path.split("\\").join("/").split("/").pop() ?? file.path;

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(undefined);
    setDocument(undefined);
    void api.readProjectFile(projectId, file.path).then((value) => {
      if (active) setDocument(value);
    }).catch((reason) => {
      if (active) setError(String(reason));
    }).finally(() => {
      if (active) setLoading(false);
    });
    return () => { active = false; };
  }, [projectId, file.path, attempt]);

  async function action(id: string, run: () => Promise<unknown>, success: MessageKey) {
    setBusy(id);
    try {
      await run();
      notify("success", t(success));
    } catch (reason) {
      notify("error", t("fileActionFailed"), String(reason));
    } finally {
      setBusy(undefined);
    }
  }

  return <Dialog.Root open onOpenChange={(open) => { if (!open) onClose(); }}>
    <Dialog.Portal>
      <Dialog.Backdrop className="dialog-backdrop" />
      <Dialog.Popup className={`dialog-popup evidence-preview-dialog${expanded ? " is-expanded" : ""}`}>
        <header className="evidence-preview-header">
          <span className="evidence-file-icon" aria-hidden="true"><FileCode weight="duotone" /></span>
          <div className="evidence-preview-title">
            <Dialog.Title className="dialog-title">{name}</Dialog.Title>
            <Dialog.Description title={file.path}>{file.path}</Dialog.Description>
          </div>
          <Button size="icon" variant="quiet" aria-label={t(expanded ? "restorePreview" : "expandPreview")} title={t(expanded ? "restorePreview" : "expandPreview")} onClick={() => setExpanded(!expanded)}>{expanded ? <ArrowsInSimple /> : <ArrowsOutSimple />}</Button>
          <Dialog.Close render={<Button size="icon" variant="quiet" aria-label={t("close")} title={t("close")}><X /></Button>} />
        </header>
        <div className="evidence-preview-toolbar">
          <span className="evidence-language">{language === "text" ? "TEXT" : language.toUpperCase()}</span>
          <div className="evidence-preview-tools">
            <Button disabled={!document || loading || !!busy} loading={busy === "content"} onClick={() => void action("content", () => navigator.clipboard.writeText(document?.content ?? ""), "copied")}><Copy />{t("copyFileContent")}</Button>
            <Button disabled={!!busy} loading={busy === "path"} onClick={() => void action("path", () => navigator.clipboard.writeText(file.path), "copied")}><Copy />{t("copyPath")}</Button>
            <Button disabled={!!busy} loading={busy === "reveal"} onClick={() => void action("reveal", () => api.revealProjectFile(projectId, file.path), "fileRevealed")}><FolderOpen />{t("revealInExplorer")}</Button>
            <Button disabled={!!busy} loading={busy === "open"} onClick={() => void action("open", () => api.openProjectFile(projectId, file.path), "fileOpened")}><ArrowSquareOut />{t("openExternally")}</Button>
          </div>
        </div>
        {file.source !== file.path && <div className="evidence-preview-source"><Code aria-hidden="true" /><span title={file.source}>{file.source}</span></div>}
        <div className="evidence-preview-body" aria-busy={loading}>
          {loading ? <div className="evidence-preview-loading" role="status"><span>{t("loadingFile")}</span><Skeleton className="skeleton-code" /></div>
            : error ? <InlineLoadError title={t("filePreviewFailed")} detail={error} retryLabel={t("retry")} onRetry={() => setAttempt((value) => value + 1)} />
              : <CodePreview code={document?.content ?? ""} language={language} />}
        </div>
        <footer className="evidence-preview-footer">
          <span>{document ? `${document.content ? document.content.split("\n").length : 0} ${t("fileLines")}` : t("filePreview")}</span>
          {document?.truncated && <span className="status-warning" role="status">{t("fileTruncated")}</span>}
          <span className="evidence-preview-escape">Esc <span>{t("close")}</span></span>
        </footer>
      </Dialog.Popup>
    </Dialog.Portal>
  </Dialog.Root>;
}
