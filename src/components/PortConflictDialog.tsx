import { Dialog } from "@base-ui/react/dialog";
import { useEffect, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import type { PortConflict } from "../types";
import { api } from "../lib/api";
import { openPortProcesses } from "../lib/port-processes";
import { Button } from "./ui/button";
import { processError } from "./PortProcesses";

interface Review { projectId: string; taskId: string; conflicts: PortConflict[]; resolve: (allow: boolean) => void }
export function PortConflictDialog({ t }: { t: (key: MessageKey) => string }) {
  const [review, setReview] = useState<Review>();
  const current = useRef<Review | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    const handler = (event: Event) => { current.current?.resolve(false); const next = (event as CustomEvent<Review>).detail; current.current = next; setReview(next); setError(""); };
    window.addEventListener("repoatlas:port-conflicts", handler);
    return () => { window.removeEventListener("repoatlas:port-conflicts", handler); current.current?.resolve(false); };
  }, []);
  function finish(allow: boolean) { current.current?.resolve(allow); current.current = undefined; setReview(undefined); }
  return <Dialog.Root open={!!review} onOpenChange={open => { if (!open && !busy) finish(false); }}><Dialog.Portal><Dialog.Backdrop className="dialog-backdrop" /><Dialog.Popup className="dialog-popup port-conflict-dialog">
    <Dialog.Title className="dialog-title">{t("portConflicts")}</Dialog.Title>
    <Dialog.Description className="dialog-description">{t("processConflictHint")}</Dialog.Description>
    <ul>{review?.conflicts.map(item => <li key={`${item.port}:${item.pid}`}>TCP {item.port} · {item.processName ?? t("processUnknown")} · PID {item.pid ?? t("processUnknown")}</li>)}</ul>
    {review?.conflicts.length === 0 && <p role="status">{t("processPortsClear")}</p>}
    {error && <p role="alert">{error}</p>}
    <div className="dialog-actions"><Button disabled={busy} onClick={() => finish(false)}>{t("cancel")}</Button>
      <Button disabled={busy} onClick={() => { if (!review) return; openPortProcesses({ projectId: review.projectId, taskId: review.taskId, ports: review.conflicts.map(p => p.port) }); finish(false); }}>{t("processLocate")}</Button>
      <Button loading={busy} onClick={async () => { if (!review || busy) return; const pending = review; setBusy(true); setError(""); try { const conflicts = await api.preflightTaskPorts(pending.projectId, pending.taskId); if (current.current === pending) { const next = { ...pending, conflicts }; current.current = next; setReview(next); } } catch (err) { setError(processError(err, t)); } finally { setBusy(false); } }}>{t("processRecheck")}</Button>
      <Button variant="primary" disabled={busy} onClick={() => finish(true)}>{review?.conflicts.length ? t("processStartAnyway") : t("run")}</Button>
    </div>
  </Dialog.Popup></Dialog.Portal></Dialog.Root>;
}
